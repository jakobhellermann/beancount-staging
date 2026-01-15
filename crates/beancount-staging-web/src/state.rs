use beancount_staging::Directive;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AppState {
    pub inner: Arc<Mutex<AppStateInner>>,
}

pub struct AppStateInner {
    pub journal_paths: Vec<PathBuf>,
    pub staging_items: Vec<Directive>,
    pub expense_accounts: HashMap<usize, String>,
}

impl AppState {
    pub fn new(journal_paths: Vec<PathBuf>, staging_paths: Vec<PathBuf>) -> anyhow::Result<Self> {
        use beancount_staging::reconcile::{ReconcileConfig, ReconcileItem};

        let results =
            ReconcileConfig::new(journal_paths.clone(), staging_paths.clone()).reconcile()?;

        // Filter only staging items
        let staging_items: Vec<Directive> = results
            .iter()
            .filter_map(|item| match item {
                ReconcileItem::OnlyInStaging(directive) => Some(directive.clone()),
                _ => None,
            })
            .collect();

        Ok(Self {
            inner: Arc::new(Mutex::new(AppStateInner {
                journal_paths,
                staging_items,
                expense_accounts: HashMap::new(),
            })),
        })
    }
}
