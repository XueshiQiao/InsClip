use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;
use std::sync::Arc;
use crate::database::Database;

static RUNNING: AtomicBool = AtomicBool::new(true);

pub fn start_clipboard_monitor(app: AppHandle, db: Arc<Database>) {
    let app_clone = app.clone();

    std::thread::spawn(move || {
        let pool = db.pool.clone();
        let mut last_text = String::new();
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

        loop {
            if !RUNNING.load(Ordering::SeqCst) {
                break;
            }

            std::thread::sleep(Duration::from_millis(1000));

            let current_text = match get_clipboard_text() {
                Ok(text) => text,
                Err(e) => {
                    eprintln!("CLIPBOARD: failed to read clipboard: {}", e);
                    continue;
                }
            };

            if current_text.is_empty() || current_text == last_text {
                continue;
            }
            
            eprintln!("CLIPBOARD: Detected new text (len: {})", current_text.len());

            last_text = current_text.clone();
            let hash = calculate_hash(current_text.as_bytes());
            let content_bytes = current_text.as_bytes().to_vec();
            let preview = current_text.chars().take(200).collect::<String>();
            let clip_uuid = Uuid::new_v4().to_string();

            // Check if exists
            let existing_uuid: Option<String> = rt.block_on(async {
                sqlx::query_scalar::<_, String>(r#"SELECT uuid FROM clips WHERE content_hash = ?"#)
                    .bind(&hash)
                    .fetch_optional(&pool)
                    .await
                    .unwrap_or(None)
            });

            if let Some(existing_id) = existing_uuid {
                eprintln!("CLIPBOARD: Found existing clip {}, updating...", existing_id);
                // Update existing clip to bring to top
                let _ = rt.block_on(async {
                    sqlx::query(r#"UPDATE clips SET created_at = CURRENT_TIMESTAMP, is_deleted = 0 WHERE uuid = ?"#)
                        .bind(&existing_id)
                        .execute(&pool)
                        .await
                });
                
                // Emit event to refresh UI
                let _ = app_clone.emit("clipboard-change", &serde_json::json!({
                    "id": existing_id,
                    "content": preview,
                    "is_pinned": false, // querying this would be better but this triggers a refresh anyway
                    "created_at": chrono::Utc::now().to_rfc3339()
                }));
                eprintln!("CLIPBOARD: Emitted change event for existing clip");
            } else {
                eprintln!("CLIPBOARD: Inserting new clip...");
                // Insert new clip
                let result = rt.block_on(async {
                    sqlx::query(r#"
                        INSERT INTO clips (uuid, clip_type, content, text_preview, content_hash, folder_id, is_pinned, is_deleted, source_app, metadata, created_at, last_accessed)
                        VALUES (?, 'text', ?, ?, ?, NULL, 0, 0, NULL, NULL, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
                    "#)
                    .bind(&clip_uuid)
                    .bind(content_bytes)
                    .bind(&preview)
                    .bind(&hash)
                    .execute(&pool)
                    .await
                });

                if let Ok(_) = result {
                    eprintln!("CLIPBOARD: Insert successful, emitting event");
                    let _ = app_clone.emit("clipboard-change", &serde_json::json!({
                        "id": clip_uuid,
                        "content": preview,
                        "is_pinned": false,
                        "created_at": chrono::Utc::now().to_rfc3339()
                    }));
                } else if let Err(e) = result {
                    eprintln!("CLIPBOARD: failed to save clip: {}", e);
                }
            }
        }
    });
}

fn get_clipboard_text() -> Result<String, arboard::Error> {
    let mut clipboard = arboard::Clipboard::new()?;
    clipboard.get_text()
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
