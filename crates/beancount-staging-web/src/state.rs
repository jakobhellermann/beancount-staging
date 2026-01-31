use beancount_parser::Account;
use beancount_staging::reconcile::{ReconcileConfig, ReconcileItem, ReconcileState, StagingSource};
use beancount_staging::{Directive, DirectiveContent};
use beancount_staging_predictor::preprocessing::Alpha;
use beancount_staging_predictor::{DecisionTreePredictor, PredictionInput, Predictor};
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::broadcast;

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
}

pub struct AppStateInner {
    pub reconcile_config: ReconcileConfig,
    pub reconcile_state: ReconcileState,

    // derived data
    pub staging_items: BTreeMap<String, Directive>,
    pub available_accounts: BTreeSet<String>,
    pub predictor: Option<DecisionTreePredictor<Alpha>>,
}

impl AppStateInner {
    fn new(journal_paths: Vec<PathBuf>, staging_source: StagingSource) -> Self {
        let reconcile_config = ReconcileConfig::new(journal_paths, staging_source);

        AppStateInner {
            reconcile_config,
            reconcile_state: ReconcileState::default(),
            staging_items: BTreeMap::new(),
            available_accounts: BTreeSet::default(),
            predictor: None,
        }
    }

    fn reload(&mut self) -> anyhow::Result<()> {
        self.reconcile_state = self.reconcile_config.read()?;
        let results = self.reconcile_state.reconcile()?;

        // Filter only staging items and build BTreeMap with unique IDs
        let mut staging_items = BTreeMap::new();
        let mut id_gen = UniqueIdGenerator::new();

        for item in &results {
            if let ReconcileItem::OnlyInStaging(directive) = item {
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
        file_change_tx: broadcast::Sender<FileChangeEvent>,
    ) -> anyhow::Result<Self> {
        let mut state = AppStateInner::new(journal_paths, staging_source);
        state.reload()?;

        Ok(Self {
            inner: Arc::new(Mutex::new(state)),
            file_change_tx,
        })
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
