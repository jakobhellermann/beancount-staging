use anyhow::Result;
use notify::{EventKind, RecursiveMode};
use notify_debouncer_full::{Debouncer, NoCache, new_debouncer};
use std::path::Path;
use std::time::Duration;
use tracing::{error, info};

pub struct FileWatcher {
    _debouncer: Debouncer<notify::RecommendedWatcher, NoCache>,
}

impl FileWatcher {
    pub fn new<'a, F>(paths: impl Iterator<Item = &'a Path>, on_change: F) -> Result<Self>
    where
        F: Fn() + Send + 'static,
    {
        let mut debouncer = new_debouncer(
            Duration::from_millis(100),
            None,
            move |res: Result<Vec<notify_debouncer_full::DebouncedEvent>, _>| {
                let mut events = match res {
                    Ok(events) => events,
                    Err(e) => {
                        error!("Watch error: {:?}", e);
                        return;
                    }
                };

                events.retain(|e| {
                    matches!(
                        e.event.kind,
                        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
                    )
                });

                if !events.is_empty() {
                    info!("File modification detected: {} events", events.len());

                    on_change();
                }
            },
        )?;

        // Watch all provided paths
        for path in paths {
            info!("Watching path: {:?}", path);
            debouncer.watch(path, RecursiveMode::NonRecursive)?;
        }

        Ok(Self {
            _debouncer: debouncer,
        })
    }
}
