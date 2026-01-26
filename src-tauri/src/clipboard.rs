use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;
use std::sync::Arc;
use crate::database::Database;
use std::io::Cursor;
use image::ImageOutputFormat;

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

            std::thread::sleep(Duration::from_millis(200));

            let mut new_clip_found = false;
            let mut clip_type = "text";
            let mut clip_content = Vec::new();
            let mut clip_preview = String::new();
            let mut clip_hash = String::new();
            let mut metadata = String::new();

            // 1. Try to get Image first
            let current_image_hash = match get_clipboard_image_hash() {
                Ok(Some((hash, bytes, _preview, meta))) => Some((hash, bytes, _preview, meta)),
                _ => None,
            };

            let current_text = get_clipboard_text().ok();

            if let Some((hash, bytes, _preview, meta)) = current_image_hash {
                if hash != last_text {
                    eprintln!("CLIPBOARD: Detected new IMAGE. Metadata: {}", meta);
                    clip_type = "image";
                    clip_content = bytes;
                    clip_preview = "[Image]".to_string();
                    clip_hash = hash.clone();
                    metadata = meta;
                    new_clip_found = true;
                    last_text = hash;
                }
            } else if let Some(text) = current_text {
                if !text.is_empty() && text != last_text {
                    eprintln!("CLIPBOARD: Detected new TEXT (len: {})", text.len());
                    clip_type = "text";
                    clip_content = text.as_bytes().to_vec();
                    clip_preview = text.chars().take(200).collect::<String>();
                    clip_hash = calculate_hash(text.as_bytes());
                    new_clip_found = true;
                    last_text = text;
                }
            }

            if !new_clip_found {
                continue;
            }

            let clip_uuid = Uuid::new_v4().to_string();

            // Check if exists
            let existing_uuid: Option<String> = rt.block_on(async {
                sqlx::query_scalar::<_, String>(r#"SELECT uuid FROM clips WHERE content_hash = ?"#)
                    .bind(&clip_hash)
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
                    "content": clip_preview,
                    "clip_type": clip_type,
                    "is_pinned": false,
                    "created_at": chrono::Utc::now().to_rfc3339()
                }));
            } else {
                eprintln!("CLIPBOARD: Inserting new clip...");
                // Insert new clip
                let result = rt.block_on(async {
                    sqlx::query(r#"
                        INSERT INTO clips (uuid, clip_type, content, text_preview, content_hash, folder_id, is_pinned, is_deleted, source_app, metadata, created_at, last_accessed)
                        VALUES (?, ?, ?, ?, ?, NULL, 0, 0, NULL, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
                    "#)
                    .bind(&clip_uuid)
                    .bind(clip_type)
                    .bind(&clip_content)
                    .bind(&clip_preview)
                    .bind(&clip_hash)
                    .bind(if clip_type == "image" { Some(metadata) } else { None })
                    .execute(&pool)
                    .await
                });

                if let Ok(_) = result {
                    eprintln!("CLIPBOARD: Insert successful, emitting event");
                    let _ = app_clone.emit("clipboard-change", &serde_json::json!({
                        "id": clip_uuid,
                        "content": clip_preview,
                        "clip_type": clip_type,
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

fn get_clipboard_image_hash() -> Result<Option<(String, Vec<u8>, String, String)>, Box<dyn std::error::Error>> {
    let mut clipboard = arboard::Clipboard::new()?;
    match clipboard.get_image() {
        Ok(image) => {
            // Convert to PNG bytes
            let width = image.width as u32;
            let height = image.height as u32;
            let image_buf = image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(width, height, image.bytes.into_owned())
                .ok_or("Failed to create image buffer")?;

            let mut bytes: Vec<u8> = Vec::new();
            let mut cursor = Cursor::new(&mut bytes);
            image_buf.write_to(&mut cursor, ImageOutputFormat::Png)?;

            let size_bytes = bytes.len();
            let hash = calculate_hash(&bytes);

            let metadata = serde_json::json!({
                "width": width,
                "height": height,
                "format": "png",
                "size_bytes": size_bytes
            }).to_string();

            Ok(Some((hash, bytes, "[Image]".to_string(), metadata)))
        },
        Err(_) => Ok(None)
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
