use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_x::{write_text, stop_listening, start_listening, write_image};

use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
use std::str::FromStr;
use crate::database::Database;
use std::sync::Arc;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use crate::models::{Clip, Folder, ClipboardItem, FolderItem};
use crate::ai::{self, AiConfig};
use dark_light::Mode;
use std::sync::atomic::Ordering;
use crate::settings_manager::SettingsManager;

#[tauri::command]
pub async fn ai_process_clip(app: AppHandle, clip_id: String, action: String, db: tauri::State<'_, Arc<Database>>) -> Result<String, String> {
    let pool = &db.pool;

    // 1. Get Clip
    let clip: Clip = sqlx::query_as(r#"SELECT * FROM clips WHERE uuid = ?"#)
        .bind(&clip_id)
        .fetch_optional(pool).await.map_err(|e| e.to_string())?
        .ok_or("Clip not found")?;

    // 2. Get Settings
    let manager = app.state::<Arc<SettingsManager>>();
    let settings = manager.get();

    let config = AiConfig {
        provider: settings.ai_provider,
        api_key: settings.ai_api_key,
        model: settings.ai_model,
        base_url: if settings.ai_base_url.is_empty() { None } else { Some(settings.ai_base_url) },
    };

    // 3. Determine Prompt & Action
    let (prompt_template, ai_action) = match action.as_str() {
        "summarize" => (settings.ai_prompt_summarize, crate::ai::AiAction::Summarize),
        "translate" => (settings.ai_prompt_translate, crate::ai::AiAction::Translate),
        "explain_code" => (settings.ai_prompt_explain_code, crate::ai::AiAction::ExplainCode),
        "fix_grammar" => (settings.ai_prompt_fix_grammar, crate::ai::AiAction::FixGrammar),
        _ => return Err("Unknown action".to_string()),
    };

    let prompt = prompt_template.replace("{{content}}", &clip.text_preview);

    // 4. Call AI
    ai::process_text(&clip.text_preview, ai_action, &config, Some(prompt)).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_clips(
    page: i64,
    page_size: i64,
    search: Option<String>,
    folder_id: Option<i64>,
    db: tauri::State<'_, Arc<Database>>
) -> Result<Vec<ClipboardItem>, String> {
    let pool = &db.pool;
    let offset = (page - 1) * page_size;

    let clips: Vec<Clip> = if let Some(q) = search {
        let pattern = format!("%{}%", q);
        sqlx::query_as(r#"
            SELECT * FROM clips
            WHERE is_deleted = 0
            AND (content LIKE ? OR text_preview LIKE ?)
            ORDER BY created_at DESC
            LIMIT ? OFFSET ?
        "#)
        .bind(&pattern)
        .bind(&pattern)
        .bind(page_size)
        .bind(offset)
        .fetch_all(pool).await.map_err(|e| e.to_string())?
    } else if let Some(fid) = folder_id {
        sqlx::query_as(r#"
            SELECT * FROM clips
            WHERE is_deleted = 0 AND folder_id = ?
            ORDER BY created_at DESC
            LIMIT ? OFFSET ?
        "#)
        .bind(fid)
        .bind(page_size)
        .bind(offset)
        .fetch_all(pool).await.map_err(|e| e.to_string())?
    } else {
        sqlx::query_as(r#"
            SELECT * FROM clips
            WHERE is_deleted = 0
            ORDER BY created_at DESC
            LIMIT ? OFFSET ?
        "#)
        .bind(page_size)
        .bind(offset)
        .fetch_all(pool).await.map_err(|e| e.to_string())?
    };

    Ok(clips.into_iter().map(|c| ClipboardItem {
        id: c.uuid,
        clip_type: c.clip_type.clone(),
        content: if c.clip_type == "image" {
             // Convert bytes to base64 for frontend
             BASE64.encode(&c.content)
        } else {
             String::from_utf8_lossy(&c.content).to_string()
        },
        preview: c.text_preview,
        folder_id: c.folder_id.map(|id| id.to_string()),
        created_at: c.created_at.to_rfc3339(),
        source_app: c.source_app,
        source_icon: c.source_icon,
        metadata: c.metadata,
    }).collect())
}

#[tauri::command]
pub async fn get_clip(uuid: String, db: tauri::State<'_, Arc<Database>>) -> Result<ClipboardItem, String> {
    let pool = &db.pool;
    let clip: Option<Clip> = sqlx::query_as(r#"SELECT * FROM clips WHERE uuid = ?"#)
        .bind(&uuid)
        .fetch_optional(pool).await.map_err(|e| e.to_string())?;

    if let Some(c) = clip {
        Ok(ClipboardItem {
            id: c.uuid,
            clip_type: c.clip_type.clone(),
            content: if c.clip_type == "image" {
                 BASE64.encode(&c.content)
            } else {
                 String::from_utf8_lossy(&c.content).to_string()
            },
            preview: c.text_preview,
            folder_id: c.folder_id.map(|id| id.to_string()),
            created_at: c.created_at.to_rfc3339(),
            source_app: c.source_app,
            source_icon: c.source_icon,
            metadata: c.metadata,
        })
    } else {
        Err("Clip not found".to_string())
    }
}

#[tauri::command]
pub async fn paste_clip(uuid: String, app: AppHandle, db: tauri::State<'_, Arc<Database>>) -> Result<(), String> {
    let pool = &db.pool;
    let clip: Option<Clip> = sqlx::query_as(r#"SELECT * FROM clips WHERE uuid = ?"#)
        .bind(&uuid)
        .fetch_optional(pool).await.map_err(|e| e.to_string())?;

    if let Some(c) = clip {
        // Temporarily stop listening to avoid capturing our own paste
        if let Err(e) = stop_listening().await {
             log::warn!("Failed to stop listening: {}", e);
        }

        // Set global ignore hash
        crate::clipboard::set_ignore_hash(c.content_hash.clone());

        let clip_type = c.clip_type.clone();

        if clip_type == "image" {
             // For image, we need to write image to clipboard
             if let Err(e) = write_image(BASE64.encode(&c.content)).await {
                 let _ = start_listening(app.clone()).await; // restart listener
                 return Err(format!("Failed to write image: {}", e));
             }
        } else {
             let text = String::from_utf8_lossy(&c.content).to_string();
             if let Err(e) = write_text(text).await {
                 let _ = start_listening(app.clone()).await; // restart listener
                 return Err(format!("Failed to write text: {}", e));
             }
        }

        // Send Paste Input
        // On Windows, write_text/image is async but fast.
        // We wait a tiny bit to ensure clipboard is updated before simulating Ctrl+V
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        crate::clipboard::send_paste_input();

        // Update Last Accessed
        let _ = sqlx::query(r#"UPDATE clips SET created_at = CURRENT_TIMESTAMP WHERE uuid = ?"#)
            .bind(&uuid)
            .execute(pool)
            .await;

        // Restart listener
        // Give some time for the paste event to propagate so we don't catch it immediately if the ignore hash logic fails
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        if let Err(e) = start_listening(app.clone()).await {
             log::warn!("Failed to restart listening: {}", e);
        }

        // Check if we should auto-hide window
        if let Some(win) = app.get_webview_window("main") {
             crate::animate_window_hide(&win, None);
        }

        Ok(())
    } else {
        Err("Clip not found".to_string())
    }
}

#[tauri::command]
pub async fn delete_clip(uuid: String, db: tauri::State<'_, Arc<Database>>) -> Result<(), String> {
    let pool = &db.pool;
    
    // Check setting: if we should delete permanently or soft delete
    // For now, let's just soft delete
    if false {
        sqlx::query(r#"DELETE FROM clips WHERE uuid = ?"#)
            .bind(&uuid)
            .execute(pool).await.map_err(|e| e.to_string())?;
    } else {
        sqlx::query(r#"UPDATE clips SET is_deleted = 1 WHERE uuid = ?"#)
            .bind(&uuid)
            .execute(pool).await.map_err(|e| e.to_string())?;
    }
    
    Ok(())
}

#[tauri::command]
pub async fn move_to_folder(uuid: String, folder_id: Option<String>, db: tauri::State<'_, Arc<Database>>) -> Result<(), String> {
    let pool = &db.pool;
    let fid = folder_id.and_then(|s| s.parse::<i64>().ok());

    sqlx::query(r#"UPDATE clips SET folder_id = ? WHERE uuid = ?"#)
        .bind(fid)
        .bind(&uuid)
        .execute(pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn create_folder(name: String, icon: Option<String>, color: Option<String>, db: tauri::State<'_, Arc<Database>>) -> Result<String, String> {
    let pool = &db.pool;

    // Check if exists
    let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM folders WHERE name = ?")
        .bind(&name)
        .fetch_optional(pool).await.map_err(|e| e.to_string())?;

    if exists.is_some() {
        return Err("Folder already exists".to_string());
    }

    let id = sqlx::query(r#"INSERT INTO folders (name, icon, color) VALUES (?, ?, ?)"#)
        .bind(&name)
        .bind(&icon)
        .bind(&color)
        .execute(pool).await.map_err(|e| e.to_string())?
        .last_insert_rowid();

    Ok(id.to_string())
}

#[tauri::command]
pub async fn delete_folder(id: String, db: tauri::State<'_, Arc<Database>>) -> Result<(), String> {
    let pool = &db.pool;
    let fid = id.parse::<i64>().map_err(|_| "Invalid ID")?;

    // Update clips in this folder to have NULL folder_id
    sqlx::query(r#"UPDATE clips SET folder_id = NULL WHERE folder_id = ?"#)
        .bind(fid)
        .execute(pool).await.map_err(|e| e.to_string())?;

    sqlx::query(r#"DELETE FROM folders WHERE id = ?"#)
        .bind(fid)
        .execute(pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn rename_folder(id: String, name: String, db: tauri::State<'_, Arc<Database>>) -> Result<(), String> {
    let pool = &db.pool;
    let fid = id.parse::<i64>().map_err(|_| "Invalid ID")?;

    // Check if name exists
    let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM folders WHERE name = ? AND id != ?")
        .bind(&name)
        .bind(fid)
        .fetch_optional(pool).await.map_err(|e| e.to_string())?;

    if exists.is_some() {
        return Err("Folder name already taken".to_string());
    }

    sqlx::query(r#"UPDATE folders SET name = ? WHERE id = ?"#)
        .bind(&name)
        .bind(fid)
        .execute(pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn search_clips(query: String, db: tauri::State<'_, Arc<Database>>) -> Result<Vec<ClipboardItem>, String> {
    let pool = &db.pool;
    let pattern = format!("%{}%", query);

    let clips: Vec<Clip> = sqlx::query_as(r#"
        SELECT * FROM clips
        WHERE is_deleted = 0
        AND (content LIKE ? OR text_preview LIKE ?)
        ORDER BY created_at DESC
        LIMIT 50
    "#)
    .bind(&pattern)
    .bind(&pattern)
    .fetch_all(pool).await.map_err(|e| e.to_string())?;

    Ok(clips.into_iter().map(|c| ClipboardItem {
        id: c.uuid,
        clip_type: c.clip_type.clone(),
        content: if c.clip_type == "image" {
             BASE64.encode(&c.content)
        } else {
             String::from_utf8_lossy(&c.content).to_string()
        },
        preview: c.text_preview,
        folder_id: c.folder_id.map(|id| id.to_string()),
        created_at: c.created_at.to_rfc3339(),
        source_app: c.source_app,
        source_icon: c.source_icon,
        metadata: c.metadata,
    }).collect())
}

#[tauri::command]
pub async fn get_folders(db: tauri::State<'_, Arc<Database>>) -> Result<Vec<FolderItem>, String> {
    let pool = &db.pool;

    let folders: Vec<Folder> = sqlx::query_as(r#"SELECT * FROM folders ORDER BY created_at"#)
        .fetch_all(pool).await.map_err(|e| e.to_string())?;

    let counts: Vec<(i64, i64)> = sqlx::query_as(r#"
        SELECT folder_id, COUNT(*) as count
        FROM clips
        WHERE is_deleted = 0 AND folder_id IS NOT NULL
        GROUP BY folder_id
    "#)
    .fetch_all(pool).await.map_err(|e| e.to_string())?;

    use std::collections::HashMap;
    let mut count_map = HashMap::new();
    for (fid, count) in counts {
        count_map.insert(fid, count);
    }

    // Add "Uncategorized" implicit folder count?
    // Frontend likely handles "All" and "Favorites".
    // We just return actual folders.

    let items = folders.into_iter().map(|folder| {
        FolderItem {
            id: folder.id.to_string(),
            name: folder.name,
            icon: folder.icon.clone(),
            color: folder.color.clone(),
            is_system: folder.is_system,
            item_count: *count_map.get(&folder.id).unwrap_or(&0),
        }
    }).collect();

    Ok(items)
}

#[tauri::command]
#[allow(unused_variables)]
pub async fn get_settings(app: AppHandle, _db: tauri::State<'_, Arc<Database>>) -> Result<serde_json::Value, String> {
    let manager = app.state::<Arc<SettingsManager>>();
    let settings = manager.get();
    let mut value = serde_json::to_value(&settings).map_err(|e| e.to_string())?;

    // Check actual autostart status and inject it
    #[cfg(not(feature = "app-store"))]
    {
        use tauri_plugin_autostart::ManagerExt;
        if let Ok(is_enabled) = app.autolaunch().is_enabled() {
            if let Some(obj) = value.as_object_mut() {
                obj.insert("startup_with_windows".to_string(), serde_json::json!(is_enabled));
            }
        }
    }

    #[cfg(all(feature = "app-store", target_os = "macos"))]
    {
        use smappservice_rs::{AppService, ServiceType, ServiceStatus};
        let app_service = AppService::new(ServiceType::MainApp);
        let is_enabled = matches!(app_service.status(), ServiceStatus::Enabled);
        if let Some(obj) = value.as_object_mut() {
            obj.insert("startup_with_windows".to_string(), serde_json::json!(is_enabled));
        }
    }

    Ok(value)
}

#[tauri::command]
pub async fn save_settings(app: AppHandle, settings: serde_json::Value, _db: tauri::State<'_, Arc<Database>>) -> Result<(), String> {
    let manager = app.state::<Arc<SettingsManager>>();
    let mut current = manager.get();

    if let Some(v) = settings.get("max_items").and_then(|v| v.as_i64()) { current.max_items = v; }
    if let Some(v) = settings.get("auto_delete_days").and_then(|v| v.as_i64()) { current.auto_delete_days = v; }
    if let Some(v) = settings.get("theme").and_then(|v| v.as_str()) { current.theme = v.to_string(); }
    if let Some(v) = settings.get("mica_effect").and_then(|v| v.as_str()) { current.mica_effect = v.to_string(); }
    if let Some(v) = settings.get("language").and_then(|v| v.as_str()) { current.language = v.to_string(); }
    if let Some(v) = settings.get("hotkey").and_then(|v| v.as_str()) { current.hotkey = v.to_string(); }
    if let Some(v) = settings.get("auto_paste").and_then(|v| v.as_bool()) { current.auto_paste = v; }
    if let Some(v) = settings.get("ignore_ghost_clips").and_then(|v| v.as_bool()) { current.ignore_ghost_clips = v; }
    
    // AI
    if let Some(v) = settings.get("ai_provider").and_then(|v| v.as_str()) { current.ai_provider = v.to_string(); }
    if let Some(v) = settings.get("ai_api_key").and_then(|v| v.as_str()) { current.ai_api_key = v.to_string(); }
    if let Some(v) = settings.get("ai_model").and_then(|v| v.as_str()) { current.ai_model = v.to_string(); }
    if let Some(v) = settings.get("ai_base_url").and_then(|v| v.as_str()) { current.ai_base_url = v.to_string(); }
    
    if let Some(v) = settings.get("ai_prompt_summarize").and_then(|v| v.as_str()) { current.ai_prompt_summarize = v.to_string(); }
    if let Some(v) = settings.get("ai_prompt_translate").and_then(|v| v.as_str()) { current.ai_prompt_translate = v.to_string(); }
    if let Some(v) = settings.get("ai_prompt_explain_code").and_then(|v| v.as_str()) { current.ai_prompt_explain_code = v.to_string(); }
    if let Some(v) = settings.get("ai_prompt_fix_grammar").and_then(|v| v.as_str()) { current.ai_prompt_fix_grammar = v.to_string(); }
    
    if let Some(v) = settings.get("ai_title_summarize").and_then(|v| v.as_str()) { current.ai_title_summarize = v.to_string(); }
    if let Some(v) = settings.get("ai_title_translate").and_then(|v| v.as_str()) { current.ai_title_translate = v.to_string(); }
    if let Some(v) = settings.get("ai_title_explain_code").and_then(|v| v.as_str()) { current.ai_title_explain_code = v.to_string(); }
    if let Some(v) = settings.get("ai_title_fix_grammar").and_then(|v| v.as_str()) { current.ai_title_fix_grammar = v.to_string(); }

    // Re-apply window effect logic
    let theme_str = current.theme.clone();
    let mica_effect = current.mica_effect.clone();
    if let Some(win) = app.get_webview_window("main") {
        let current_theme = if theme_str == "light" {
            tauri::Theme::Light
        } else if theme_str == "dark" {
            tauri::Theme::Dark
        } else {
            let mode = dark_light::detect().map_err(|e| e.to_string())?;
            match mode {
                Mode::Dark => tauri::Theme::Dark,
                _ => tauri::Theme::Light,
            }
        };
        crate::apply_window_effect(&win, &mica_effect, &current_theme);
    }

    // Autostart Logic
    #[cfg(not(feature = "app-store"))]
    {
        use tauri_plugin_autostart::ManagerExt;
        if let Some(startup) = settings.get("startup_with_windows").and_then(|v| v.as_bool()) {
            current.startup_with_windows = startup;
            let current_state = app.autolaunch().is_enabled().unwrap_or(false);
            if startup != current_state {
                if startup {
                    let _ = app.autolaunch().enable();
                } else {
                    let _ = app.autolaunch().disable();
                }
            }
        }
    }
    #[cfg(all(feature = "app-store", target_os = "macos"))]
    {
        if let Some(startup) = settings.get("startup_with_windows").and_then(|v| v.as_bool()) {
            current.startup_with_windows = startup;
            use smappservice_rs::{AppService, ServiceType, ServiceStatus};
            let app_service = AppService::new(ServiceType::MainApp);
            let current_state = matches!(app_service.status(), ServiceStatus::Enabled);
            if startup != current_state {
                if startup {
                    let _ = app_service.register();
                } else {
                    let _ = app_service.unregister();
                }
            }
        }
    }

    manager.save(current)?;
    Ok(())
}

#[tauri::command]
pub fn hide_window(window: tauri::WebviewWindow) -> Result<(), String> {
    window.hide().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ping() -> Result<String, String> {
    Ok("pong".to_string())
}

#[tauri::command]
pub fn test_log() -> Result<String, String> {
    log::trace!("[TEST] Trace level log");
    log::debug!("[TEST] Debug level log");
    log::info!("[TEST] Info level log");
    log::warn!("[TEST] Warn level log");
    log::error!("[TEST] Error level log");
    Ok("Logs emitted - check console".to_string())
}

#[tauri::command]
pub async fn get_clipboard_history_size(db: tauri::State<'_, Arc<Database>>) -> Result<i64, String> {
    let pool = &db.pool;

    let count: i64 = sqlx::query_scalar::<_, i64>(r#"SELECT COUNT(*) FROM clips WHERE is_deleted = 0"#)
        .fetch_one(pool).await.map_err(|e| e.to_string())?;
    Ok(count)
}

#[tauri::command]
pub async fn clear_clipboard_history(db: tauri::State<'_, Arc<Database>>) -> Result<(), String> {
    let pool = &db.pool;

    sqlx::query(r#"DELETE FROM clips WHERE is_deleted = 1"#)
        .execute(pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn clear_all_clips(db: tauri::State<'_, Arc<Database>>) -> Result<(), String> {
    let pool = &db.pool;

    sqlx::query(r#"DELETE FROM clips"#)
        .execute(pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn remove_duplicate_clips(db: tauri::State<'_, Arc<Database>>) -> Result<i64, String> {
    let pool = &db.pool;

    let result = sqlx::query(r#"
        DELETE FROM clips
        WHERE id NOT IN (
            SELECT MIN(id)
            FROM clips
            GROUP BY content_hash
        )
    "#)
    .execute(pool).await.map_err(|e| e.to_string())?;

    Ok(result.rows_affected() as i64)
}

#[tauri::command]
pub async fn register_global_shortcut(hotkey: String, window: tauri::WebviewWindow) -> Result<(), String> {
    use tauri_plugin_global_shortcut::ShortcutState;

    let app = window.app_handle();
    let shortcut = Shortcut::from_str(&hotkey).map_err(|e| format!("Invalid hotkey: {:?}", e))?;

    if let Err(e) = app.global_shortcut().unregister_all() {
        log::warn!("Failed to unregister existing shortcuts: {:?}", e);
    }

    let main_window = app.get_webview_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;

    let win_clone = main_window.clone();
    if let Err(e) = app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, event| {
        if event.state() == ShortcutState::Pressed {
            crate::position_window_at_bottom(&win_clone);
        }
    }) {
        return Err(format!("Failed to register hotkey: {:?}", e));
    }

    log::info!("Registered global shortcut: {}", hotkey);
    Ok(())
}

#[tauri::command]
pub async fn focus_window(app: AppHandle, label: String) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(&label) {
        if let Err(e) = window.unminimize() {
            log::warn!("Failed to unminimize window {}: {:?}", label, e);
        }
        if let Err(e) = window.show() {
            log::warn!("Failed to show window {}: {:?}", label, e);
        }
        if let Err(e) = window.set_focus() {
            log::warn!("Failed to focus window {}: {:?}", label, e);
        }

        Ok(())
    } else {
        Err(format!("Window {} not found", label))
    }
}

#[tauri::command]
pub fn show_window(window: tauri::WebviewWindow) -> Result<(), String> {
    crate::position_window_at_bottom(&window);
    Ok(())
}

#[tauri::command]
pub async fn add_ignored_app(app_name: String, app: AppHandle) -> Result<(), String> {
    let manager = app.state::<Arc<SettingsManager>>();
    let mut current = manager.get();
    if current.ignored_apps.insert(app_name) {
        manager.save(current)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn remove_ignored_app(app_name: String, app: AppHandle) -> Result<(), String> {
    let manager = app.state::<Arc<SettingsManager>>();
    let mut current = manager.get();
    if current.ignored_apps.remove(&app_name) {
        manager.save(current)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_ignored_apps(app: AppHandle) -> Result<Vec<String>, String> {
    let manager = app.state::<Arc<SettingsManager>>();
    let mut apps: Vec<String> = manager.get().ignored_apps.into_iter().collect();
    apps.sort();
    Ok(apps)
}

#[tauri::command]
pub async fn pick_file(app: AppHandle) -> Result<String, String> {
    use tauri_plugin_dialog::DialogExt;

    let file_path = app
        .dialog()
        .file()
        .add_filter("Executables", &["exe", "app"])
        .blocking_pick_file();

    match file_path {
        Some(path) => Ok(path.to_string()),
        None => Err("No file selected".to_string()),
    }
}

#[tauri::command]
pub fn get_layout_config() -> serde_json::Value {
    serde_json::json!({
        "window_height": crate::constants::WINDOW_HEIGHT,
    })
}

#[tauri::command]
pub async fn check_accessibility_permissions() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        Ok(crate::source_app_macos::is_accessibility_enabled())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(true)
    }
}

#[tauri::command]
pub async fn request_accessibility_permissions() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        crate::source_app_macos::open_accessibility_settings();
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(())
    }
}
