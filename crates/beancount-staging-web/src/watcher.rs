use anyhow::Result;
use notify::{EventKind, RecursiveMode};
use notify_debouncer_full::{Debouncer, RecommendedCache, new_debouncer};
use std::path::Path;
use std::time::Duration;
use tracing::{error, info};

pub struct FileWatcher {
    _debouncer: Debouncer<notify::RecommendedWatcher, RecommendedCache>,
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
            let path_display = (|| {
                let cwd = std::env::current_dir().ok()?;
                let base = path.strip_prefix(&cwd).ok()?;
                Some(base)
            })()
            .unwrap_or(path);
            info!("Watching path: {}", path_display.display());
            debouncer.watch(path, RecursiveMode::NonRecursive)?;
        }

        Ok(Self {
            _debouncer: debouncer,
        })
    }
}
