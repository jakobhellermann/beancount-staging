use beancount_staging::Directive;
use beancount_staging::reconcile::{ReconcileConfig, ReconcileItem};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

#[derive(Clone, Debug)]
pub struct FileChangeEvent;

#[derive(Clone)]
pub struct AppState {
    pub inner: Arc<Mutex<AppStateInner>>,
    pub file_change_tx: broadcast::Sender<FileChangeEvent>,
}

pub struct AppStateInner {
    pub reconcile_config: ReconcileConfig,
    pub staging_items: Vec<Directive>,
    pub expense_accounts: HashMap<usize, String>,
}

impl AppStateInner {
    fn reload(&mut self) -> anyhow::Result<()> {
        let results = self.reconcile_config.reconcile()?;

        // Filter only staging items
        let staging_items: Vec<Directive> = results
            .iter()
            .filter_map(|item| match item {
                ReconcileItem::OnlyInStaging(directive) => Some(directive.clone()),
                _ => None,
            })
            .collect();

        self.staging_items = staging_items;
        // Keep existing expense_accounts for transactions that still exist

        Ok(())
    }
}

impl AppState {
    pub fn new(
        journal_paths: Vec<PathBuf>,
        staging_paths: Vec<PathBuf>,
        file_change_tx: broadcast::Sender<FileChangeEvent>,
    ) -> anyhow::Result<Self> {
        let reconcile_config = ReconcileConfig::new(journal_paths, staging_paths);

        let mut state = AppStateInner {
            reconcile_config,
            staging_items: Vec::new(),
            expense_accounts: HashMap::new(),
        };
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
