use anyhow::Result;
use notify::{Watcher, RecursiveMode, Event, EventKind};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use crate::config::AespConfig;
use crate::schema::Schema;
use crate::storage::Storage;

pub fn start_watcher(
    project_root: PathBuf,
    storage: &Storage,
    schema: &Schema,
    config: &AespConfig,
) -> Result<()> {
    let (tx, rx) = mpsc::channel();
    let debounce_ms = config.watcher.debounce_ms;

    let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
        if let Ok(event) = res {
            let _ = tx.send(event);
        }
    })?;

    watcher.watch(&project_root, RecursiveMode::Recursive)?;

    tracing::info!("File watcher started for: {}", project_root.display());

    let mut pending_paths: Vec<PathBuf> = Vec::new();
    let mut last_event = std::time::Instant::now();

    loop {
        match rx.recv_timeout(Duration::from_millis(debounce_ms)) {
            Ok(event) => {
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                        for path in event.paths {
                            if !should_ignore_watch(&path, &project_root) {
                                if !pending_paths.contains(&path) {
                                    pending_paths.push(path);
                                }
                            }
                        }
                        last_event = std::time::Instant::now();
                    }
                    _ => {}
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if !pending_paths.is_empty()
                    && last_event.elapsed() > Duration::from_millis(debounce_ms)
                {
                    for path in pending_paths.drain(..) {
                        tracing::info!("Reindexing: {}", path.display());
                        let _ = crate::indexer::index_path(
                            &project_root,
                            &path,
                            storage,
                            schema,
                            config,
                        );
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }

    Ok(())
}

fn should_ignore_watch(path: &Path, project_root: &Path) -> bool {
    let relative = path.strip_prefix(project_root).unwrap_or(path);
    for component in relative.components() {
        let name = component.as_os_str().to_string_lossy();
        for ignored in crate::config::BUILTIN_IGNORE_DIRS {
            if name == *ignored {
                return true;
            }
        }
    }

    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        for ignored_ext in crate::config::BUILTIN_IGNORE_EXTENSIONS {
            if ext == *ignored_ext {
                return true;
            }
        }
    }

    false
}
