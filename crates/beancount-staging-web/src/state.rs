use beancount_parser::Account;
use beancount_staging::reconcile::{
    ReconcileConfig, ReconcileItemKind, ReconcileState, StagingSource,
};
use beancount_staging::{AutoCategorizeRule, Directive, DirectiveContent};
use beancount_staging_predictor::preprocessing::Alpha;
use beancount_staging_predictor::{DecisionTreePredictor, PredictionInput, Predictor};
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::broadcast;

use crate::watcher::FileWatcher;

/// Generates unique IDs for directives, handling collisions by adding counter suffixes
struct UniqueIdGenerator {
    id_counters: HashMap<String, usize>,
}

impl UniqueIdGenerator {
    fn new() -> Self {
        Self {
            id_counters: HashMap::new(),
        }
    }

    fn generate_id(&mut self, directive: &Directive) -> String {
        let base_id = Self::generate_directive_id(directive);
        let counter = self.id_counters.entry(base_id.clone()).or_insert(0);
        *counter += 1;

        match *counter {
            1 => base_id,
            _ => format!("{}-{}", base_id, counter),
        }
    }

    fn generate_directive_id(directive: &Directive) -> String {
        let mut hasher = DefaultHasher::new();

        // Hash the date
        directive.date.to_string().hash(&mut hasher);

        // Hash transaction-specific data
        if let DirectiveContent::Transaction(txn) = &directive.content {
            if let Some(payee) = &txn.payee {
                payee.hash(&mut hasher);
            }
            if let Some(narration) = &txn.narration {
                narration.hash(&mut hasher);
            }

            // Hash all posting amounts
            for posting in &txn.postings {
                if let Some(amount) = &posting.amount {
                    amount.value.to_string().hash(&mut hasher);
                    amount.currency.to_string().hash(&mut hasher);
                }
            }
        }

        let hash = hasher.finish();
        let hash_str = format!("{:08x}", hash & 0xFFFFFFFF); // Take first 8 hex chars

        format!("{}-{}", directive.date, hash_str)
    }
}

fn train_predictor(reconcile_state: &ReconcileState) -> Option<DecisionTreePredictor<Alpha>> {
    use beancount_staging_predictor::training::extract_training_examples;

    // Extract training examples from journal directives
    let examples = extract_training_examples(&reconcile_state.journal);

    // Require minimum training data
    const MIN_TRAINING_EXAMPLES: usize = 10;
    if examples.len() < MIN_TRAINING_EXAMPLES {
        tracing::warn!(
            "Not enough training examples ({} < {}), skipping predictor training",
            examples.len(),
            MIN_TRAINING_EXAMPLES
        );
        return None;
    }

    let start = Instant::now();

    // Train the predictor
    let predictor = DecisionTreePredictor::<Alpha>::train(&examples);
    tracing::info!(
        "Training predictor with {} examples took {:?}",
        examples.len(),
        start.elapsed()
    );

    Some(predictor)
}

#[derive(Clone, Debug)]
pub struct FileChangeEvent;

#[derive(Clone)]
pub struct AppState {
    pub inner: Arc<Mutex<AppStateInner>>,
    pub file_change_tx: broadcast::Sender<FileChangeEvent>,
    /// FileWatcher must be kept alive for the duration of the application.
    /// It's stored here to prevent it from being dropped.
    _watcher: Option<Arc<FileWatcher>>,
}

pub struct AppStateInner {
    pub reconcile_config: ReconcileConfig,
    pub reconcile_state: ReconcileState,
    pub auto_rules: Vec<AutoCategorizeRule>,

    // derived data
    pub staging_items: BTreeMap<String, Directive>,
    pub available_accounts: BTreeSet<String>,
    pub predictor: Option<DecisionTreePredictor<Alpha>>,
}

/// Why an `OnlyInStaging` directive may be auto-committed.
#[derive(Debug)]
enum AutoCommitDecision<'a> {
    /// A user-configured `[[auto_categorize]]` rule matched. The wrapped
    /// account becomes the balancing posting.
    Rule(&'a str),
    /// The directive is unambiguous on its own (a `*`-flagged balanced
    /// transaction, or a balance directive — both commit verbatim).
    AcceptAsIs,
}

impl AutoCommitDecision<'_> {
    /// The expense-account override to pass to `commit_transaction`, if any.
    fn target_account(&self) -> Option<&str> {
        match self {
            AutoCommitDecision::Rule(account) => Some(account),
            AutoCommitDecision::AcceptAsIs => None,
        }
    }
}

impl AppStateInner {
    fn new(
        journal_paths: Vec<PathBuf>,
        staging_source: StagingSource,
        auto_rules: Vec<AutoCategorizeRule>,
    ) -> Self {
        let reconcile_config = ReconcileConfig::new(journal_paths, staging_source);

        AppStateInner {
            reconcile_config,
            reconcile_state: ReconcileState::default(),
            auto_rules,
            staging_items: BTreeMap::new(),
            available_accounts: BTreeSet::default(),
            predictor: None,
        }
    }

    fn reload(&mut self) -> anyhow::Result<()> {
        self.reconcile_state = self.reconcile_config.read()?;
        let results = self.reconcile_state.reconcile()?;

        if self.auto_commit_staging(&results) > 0 {
            // Re-read so newly-committed transactions show up as journal-matched
            // and are filtered out of the UI list below.
            self.reconcile_state = self.reconcile_config.read()?;
        }
        let results = self.reconcile_state.reconcile()?;

        // Filter only staging items and build BTreeMap with unique IDs
        let mut staging_items = BTreeMap::new();
        let mut id_gen = UniqueIdGenerator::new();

        for item in &results {
            if let ReconcileItemKind::OnlyInStaging(directive) = item.item {
                let unique_id = id_gen.generate_id(directive);
                staging_items.insert(unique_id, (*directive).clone());
            }
        }

        self.staging_items = staging_items;

        // Extract all available accounts from journal
        self.available_accounts = self.reconcile_state.accounts();

        // Note: We don't retrain the predictor on every reload since it's expensive
        // and the journal changes frequently (on every commit). The predictor is only
        // trained once at startup and can use slightly stale data.

        Ok(())
    }

    /// For each `OnlyInStaging` item, auto-commit it if either
    /// (a) a user-configured rule matches, or
    /// (b) the transaction is non-`!`-flagged and already balanced.
    /// Logs a summary and returns the number of successful commits.
    fn auto_commit_staging(
        &self,
        results: &[beancount_staging::reconcile::ReconcileItem<'_>],
    ) -> usize {
        let Some(journal_path) = self.reconcile_config.journal_paths.first() else {
            return 0;
        };
        let mut committed_lines: Vec<String> = Vec::new();
        for item in results {
            let ReconcileItemKind::OnlyInStaging(directive) = item.item else {
                continue;
            };
            let Some(decision) = self.decide_auto_commit(directive) else {
                continue;
            };

            let (label, amount, target_desc) = match &directive.content {
                DirectiveContent::Transaction(txn) => {
                    // Prefer payee for the log; fall back to narration since
                    // pre-balanced imports often have only narration.
                    let label = txn
                        .payee
                        .as_deref()
                        .or(txn.narration.as_deref())
                        .unwrap_or("")
                        .to_string();
                    let amount = txn
                        .postings
                        .first()
                        .and_then(|p| p.amount.as_ref())
                        .map(|a| format!("{} {}", a.value, a.currency))
                        .unwrap_or_else(|| "?".to_string());
                    let target_desc = match &decision {
                        AutoCommitDecision::Rule(account) => account.to_string(),
                        AutoCommitDecision::AcceptAsIs => "(pre-balanced)".to_string(),
                    };
                    (label, amount, target_desc)
                }
                DirectiveContent::Balance(bal) => {
                    let label = bal.account.to_string();
                    let amount = format!("{} {}", bal.amount.value, bal.amount.currency);
                    (label, amount, "(balance)".to_string())
                }
                _ => continue,
            };
            if let Err(e) = beancount_staging::commit_transaction(
                directive,
                decision.target_account(),
                None,
                None,
                beancount_staging::SourceMetaTarget::Transaction,
                journal_path,
            ) {
                tracing::error!("Failed to auto-commit directive ({:?}): {}", label, e);
            } else {
                committed_lines.push(format!(
                    "{} {:?} ({}) -> {}",
                    directive.date, label, amount, target_desc,
                ));
            }
        }
        if !committed_lines.is_empty() {
            tracing::info!(
                "Auto-categorized {} transactions:\n  {}",
                committed_lines.len(),
                committed_lines.join("\n  "),
            );
        }
        committed_lines.len()
    }

    /// Decide whether a staging directive should be auto-committed.
    fn decide_auto_commit<'a>(&'a self, directive: &Directive) -> Option<AutoCommitDecision<'a>> {
        if let Some(rule) = beancount_staging::find_matching_rule(directive, &self.auto_rules) {
            return Some(AutoCommitDecision::Rule(&rule.target_account));
        }
        match &directive.content {
            DirectiveContent::Transaction(txn)
                if txn.flag != Some('!') && beancount_staging::is_transaction_balanced(txn) =>
            {
                Some(AutoCommitDecision::AcceptAsIs)
            }
            // Balance directives carry no ambiguity (they only match if
            // identical), so we always auto-commit them.
            DirectiveContent::Balance(_) => Some(AutoCommitDecision::AcceptAsIs),
            _ => None,
        }
    }

    pub fn retrain(&mut self) -> anyhow::Result<()> {
        self.predictor = train_predictor(&self.reconcile_state);
        Ok(())
    }

    pub fn predict(&self, directive: &Directive) -> Option<Account> {
        let Some(predictor) = &self.predictor else {
            return None;
        };

        let DirectiveContent::Transaction(txn) = &directive.content else {
            return None;
        };
        // TODO: handle source account in second posting?
        let source_account = txn
            .postings
            .first()
            .map(|p| p.account.clone())
            .unwrap_or_else(|| "Assets:Unknown".parse().unwrap());

        let input = PredictionInput {
            source_account,
            payee: txn.payee.clone(),
            narration: txn.narration.clone().unwrap_or_default(),
        };

        predictor.predict(&input)
    }
}

impl AppState {
    pub fn lock(
        &self,
    ) -> Result<
        std::sync::MutexGuard<'_, AppStateInner>,
        std::sync::PoisonError<std::sync::MutexGuard<'_, AppStateInner>>,
    > {
        self.inner.lock()
    }

    pub fn new(
        journal_paths: Vec<PathBuf>,
        staging_source: StagingSource,
        auto_rules: Vec<AutoCategorizeRule>,
        file_change_tx: broadcast::Sender<FileChangeEvent>,
    ) -> anyhow::Result<Self> {
        let mut state = AppStateInner::new(journal_paths, staging_source, auto_rules);
        state.reload()?;

        Ok(Self {
            inner: Arc::new(Mutex::new(state)),
            file_change_tx,
            _watcher: None,
        })
    }

    pub fn set_watcher(&mut self, watcher: FileWatcher) {
        self._watcher = Some(Arc::new(watcher));
    }

    pub fn reload(&self) -> anyhow::Result<()> {
        let mut inner = self.inner.lock().unwrap();
        inner.reload()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_transaction(date: &str, payee: &str, narration: &str, amount: &str) -> Directive {
        let txn = format!(
            r#"{date} * "{payee}" "{narration}"
  Assets:Checking  {amount} USD
"#
        );
        let parsed = beancount_parser::parse::<beancount_staging::Decimal>(&txn).unwrap();
        parsed.directives.into_iter().next().unwrap()
    }

    #[test]
    fn unique_id_generator_no_collisions() {
        let mut id_gen = UniqueIdGenerator::new();

        let txn1 = make_transaction("2024-01-01", "Store A", "Purchase", "10.00");
        let txn2 = make_transaction("2024-01-02", "Store B", "Purchase", "20.00");
        let txn3 = make_transaction("2024-01-03", "Store C", "Purchase", "30.00");

        let id1 = id_gen.generate_id(&txn1);
        let id2 = id_gen.generate_id(&txn2);
        let id3 = id_gen.generate_id(&txn3);

        // All IDs should be different base IDs without suffixes
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);

        // None should have a counter suffix
        assert!(!id1.ends_with("-2"));
        assert!(!id2.ends_with("-2"));
        assert!(!id3.ends_with("-2"));
    }

    #[test]
    fn unique_id_generator_with_collisions() {
        let mut id_gen = UniqueIdGenerator::new();

        // Create 4 identical transactions
        let txn1 = make_transaction("2024-01-01", "Store", "Purchase", "10.00");
        let txn2 = make_transaction("2024-01-01", "Store", "Purchase", "10.00");
        let txn3 = make_transaction("2024-01-01", "Store", "Purchase", "10.00");
        let txn4 = make_transaction("2024-01-01", "Store", "Purchase", "10.00");

        let id1 = id_gen.generate_id(&txn1);
        let id2 = id_gen.generate_id(&txn2);
        let id3 = id_gen.generate_id(&txn3);
        let id4 = id_gen.generate_id(&txn4);

        // First should have no suffix
        assert!(!id1.ends_with("-2"));
        assert!(!id1.ends_with("-3"));
        assert!(!id1.ends_with("-4"));

        // Subsequent ones should have counter suffixes
        assert_eq!(id2, format!("{}-2", id1));
        assert_eq!(id3, format!("{}-3", id1));
        assert_eq!(id4, format!("{}-4", id1));

        // All IDs should be unique
        let ids = vec![&id1, &id2, &id3, &id4];
        let unique_ids: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique_ids.len(), 4);
    }

    #[test]
    fn unique_id_generator_mixed_collisions() {
        let mut id_gen = UniqueIdGenerator::new();

        let txn1 = make_transaction("2024-01-01", "Store A", "Purchase", "10.00");
        let txn2 = make_transaction("2024-01-01", "Store A", "Purchase", "10.00"); // duplicate
        let txn3 = make_transaction("2024-01-02", "Store B", "Purchase", "20.00"); // different
        let txn4 = make_transaction("2024-01-01", "Store A", "Purchase", "10.00"); // duplicate again

        let id1 = id_gen.generate_id(&txn1);
        let id2 = id_gen.generate_id(&txn2);
        let id3 = id_gen.generate_id(&txn3);
        let id4 = id_gen.generate_id(&txn4);

        // First occurrence of each unique transaction should have no suffix
        assert!(!id1.ends_with("-2"));
        assert!(!id3.ends_with("-2"));

        // Duplicates should have suffixes
        assert_eq!(id2, format!("{}-2", id1));
        assert_eq!(id4, format!("{}-3", id1));

        // id3 should be different from id1
        assert_ne!(id3, id1);
        assert!(!id3.starts_with(&id1));
    }
}
