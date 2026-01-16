use beancount_staging::Directive;
use beancount_staging::reconcile::{ReconcileConfig, ReconcileItem, ReconcileState};
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

pub fn generate_directive_id(directive: &Directive) -> String {
    use beancount_parser::DirectiveContent;

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
}

impl AppStateInner {
    fn new(journal_paths: Vec<PathBuf>, staging_paths: Vec<PathBuf>) -> Self {
        let reconcile_config = ReconcileConfig::new(journal_paths, staging_paths);

        AppStateInner {
            reconcile_config,
            reconcile_state: ReconcileState::default(),
            staging_items: BTreeMap::new(),
            available_accounts: BTreeSet::default(),
        }
    }

    fn reload(&mut self) -> anyhow::Result<()> {
        self.reconcile_state = self.reconcile_config.read()?;
        let results = self.reconcile_state.reconcile()?;

        // Filter only staging items and build BTreeMap (automatically sorted by key)
        let staging_items: BTreeMap<String, Directive> = results
            .iter()
            .filter_map(|item| match *item {
                ReconcileItem::OnlyInStaging(directive) => {
                    let id = generate_directive_id(directive);
                    Some((id, directive.clone()))
                }
                _ => None,
            })
            .collect();

        self.staging_items = staging_items;

        // Extract all available accounts from journal
        self.available_accounts = self.reconcile_state.accounts();

        Ok(())
    }
}

impl AppState {
    pub fn new(
        journal_paths: Vec<PathBuf>,
        staging_paths: Vec<PathBuf>,
        file_change_tx: broadcast::Sender<FileChangeEvent>,
    ) -> anyhow::Result<Self> {
        let mut state = AppStateInner::new(journal_paths, staging_paths);
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
