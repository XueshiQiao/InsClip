use tauri::{AppHandle, Manager};
use sha2::{Sha256, Digest};
use std::sync::atomic::{AtomicBool, Ordering};
use uuid::Uuid;
use std::time::Duration;
use std::collections::HashSet;

static RUNNING: AtomicBool = AtomicBool::new(true);

pub fn start_clipboard_monitor(app: AppHandle) {
    std::thread::spawn(move || {
        let mut seen_hashes: HashSet<String> = HashSet::new();
        let mut last_text = String::new();

        loop {
            if !RUNNING.load(Ordering::SeqCst) {
                break;
            }

            std::thread::sleep(Duration::from_millis(500));

            let current_text = get_clipboard_text();

            if current_text != last_text && !current_text.is_empty() {
                last_text = current_text.clone();

                let hash = calculate_hash(current_text.as_bytes());

                if !seen_hashes.contains(&hash) {
                    seen_hashes.insert(hash.clone());

                    if let Some(app_state) = app.state::<super::APP_STATE>().get() {
                        if let Ok(mut db) = app_state.database.lock() {
                            let existing = std::thread::spawn(move || {
                                futures::executor::block_on(async {
                                    db.get_clip_by_hash(&hash).await.ok().flatten()
                                })
                            }).join().unwrap();

                            if existing.is_none() {
                                let clip = super::models::Clip {
                                    id: 0,
                                    uuid: Uuid::new_v4().to_string(),
                                    clip_type: "text".to_string(),
                                    content: current_text.as_bytes().to_vec(),
                                    text_preview: current_text.chars().take(200).collect(),
                                    content_hash: hash,
                                    folder_id: None,
                                    is_pinned: false,
                                    is_deleted: false,
                                    source_app: None,
                                    metadata: None,
                                    created_at: chrono::Utc::now(),
                                    last_accessed: chrono::Utc::now(),
                                };

                                std::thread::spawn(move || {
                                    futures::executor::block_on(async {
                                        let _ = db.add_clip(&clip).await;
                                    })
                                }).join().ok();
                            }
                        }

                        let _ = app.emit("clipboard-change", &clip);
                    }
                }
            }

            if let Some(app_state) = app.state::<super::APP_STATE>().get() {
                if let Ok(db) = app_state.database.lock() {
                    let clips_count = futures::executor::block_on(async {
                        db.get_clipboard_history_size().await.unwrap_or(0)
                    });

                    if clips_count > 1000 {
                        std::thread::spawn(move || {
                            futures::executor::block_on(async {
                                db.delete_old_clips(30).await.ok();
                            })
                        }).join().ok();
                    }
                }
            }
        }
    });
}

fn get_clipboard_text() -> String {
    use std::process::Command;

    let output = Command::new("powershell")
        .args(&["-Command", "Get-Clipboard"])
        .output();

    match output {
        Ok(output) => {
            if output.status.success() {
                String::from_utf8_lossy(&output.stdout)
                    .to_string()
                    .trim()
                    .to_string()
            } else {
                String::new()
            }
        }
        Err(_) => String::new(),
    }
}

fn calculate_hash(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    let result = hasher.finalize();
    format!("{:x}", result)
}

pub fn stop_clipboard_monitor() {
    RUNNING.store(false, Ordering::SeqCst);
}
