use tauri::{State, AppHandle};
use serde::Serialize;
use crate::models::{AppState, ClipboardItem, FolderItem};

#[tauri::command]
pub async fn get_clips(
    folder_id: Option<String>,
    limit: i64,
    offset: i64,
    state: State<'static, AppState>,
) -> Result<Vec<ClipboardItem>, String> {
    let inner = state.inner.lock().map_err(|e| e.to_string())?;
    let db = &inner.database;

    let folder_id = match folder_id {
        Some(id) => Some(id.parse::<i64>().map_err(|_| "Invalid folder ID")?),
        None => None,
    };

    let clips = futures::executor::block_on(async {
        db.get_clips(folder_id, limit, offset).await
    }).map_err(|e| e.to_string())?;

    let items: Vec<ClipboardItem> = clips.iter().map(|clip| {
        let content_str = String::from_utf8_lossy(&clip.content).to_string();

        ClipboardItem {
            id: clip.uuid.clone(),
            clip_type: clip.clip_type.clone(),
            content: content_str.clone(),
            preview: clip.text_preview.clone(),
            is_pinned: clip.is_pinned,
            folder_id: clip.folder_id.map(|id| id.to_string()),
            created_at: clip.created_at.to_rfc3339(),
            source_app: clip.source_app.clone(),
        }
    }).collect();

    Ok(items)
}

#[tauri::command]
pub async fn get_clip(
    id: String,
    state: State<'static, AppState>,
) -> Result<ClipboardItem, String> {
    let inner = state.inner.lock().map_err(|e| e.to_string())?;
    let db = &inner.database;

    let clip = futures::executor::block_on(async {
        db.get_clip_by_uuid(&id).await
    }).map_err(|e| e.to_string())?;

    match clip {
        Some(clip) => {
            let content_str = String::from_utf8_lossy(&clip.content).to_string();

            Ok(ClipboardItem {
                id: clip.uuid,
                clip_type: clip.clip_type,
                content: content_str,
                preview: clip.text_preview,
                is_pinned: clip.is_pinned,
                folder_id: clip.folder_id.map(|id| id.to_string()),
                created_at: clip.created_at.to_rfc3339(),
                source_app: clip.source_app,
            })
        }
        None => Err("Clip not found".to_string()),
    }
}

#[tauri::command]
pub async fn paste_clip(
    id: String,
    state: State<'static, AppState>,
) -> Result<(), String> {
    let inner = state.inner.lock().map_err(|e| e.to_string())?;
    let db = &inner.database;

    let clip = futures::executor::block_on(async {
        db.get_clip_by_uuid(&id).await
    }).map_err(|e| e.to_string())?;

    match clip {
        Some(clip) => {
            let content = String::from_utf8_lossy(&clip.content).to_string();
            let _ = tauri_plugin_clipboard_manager::ClipboardExt::write_text(&tauri::AppHandle::default(), content);
            Ok(())
        }
        None => Err("Clip not found".to_string()),
    }
}

#[tauri::command]
pub async fn delete_clip(
    id: String,
    hard_delete: bool,
    state: State<'static, AppState>,
) -> Result<(), String> {
    let inner = state.inner.lock().map_err(|e| e.to_string())?;
    let db = &inner.database;

    if hard_delete {
        let clip = futures::executor::block_on(async {
            db.get_clip_by_uuid(&id).await
        }).map_err(|e| e.to_string())?;
        if let Some(clip) = clip {
            futures::executor::block_on(async {
                db.hard_delete_clip(clip.id).await.map_err(|e| e.to_string())?;
                Ok::<(), String>(())
            })?;
        }
    } else {
        let clip = futures::executor::block_on(async {
            db.get_clip_by_uuid(&id).await
        }).map_err(|e| e.to_string())?;
        if let Some(clip) = clip {
            futures::executor::block_on(async {
                db.delete_clip(clip.id).await.map_err(|e| e.to_string())?;
                Ok::<(), String>(())
            })?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn pin_clip(
    id: String,
    state: State<'static, AppState>,
) -> Result<(), String> {
    let inner = state.inner.lock().map_err(|e| e.to_string())?;
    let db = &inner.database;

    let clip = futures::executor::block_on(async {
        db.get_clip_by_uuid(&id).await
    }).map_err(|e| e.to_string())?;

    match clip {
        Some(clip) => {
            futures::executor::block_on(async {
                db.pin_clip(clip.id).await.map_err(|e| e.to_string())?;
                Ok::<(), String>(())
            })?;
            Ok(())
        }
        None => Err("Clip not found".to_string()),
    }
}

#[tauri::command]
pub async fn unpin_clip(
    id: String,
    state: State<'static, AppState>,
) -> Result<(), String> {
    let inner = state.inner.lock().map_err(|e| e.to_string())?;
    let db = &inner.database;

    let clip = futures::executor::block_on(async {
        db.get_clip_by_uuid(&id).await
    }).map_err(|e| e.to_string())?;

    match clip {
        Some(clip) => {
            futures::executor::block_on(async {
                db.unpin_clip(clip.id).await.map_err(|e| e.to_string())?;
                Ok::<(), String>(())
            })?;
            Ok(())
        }
        None => Err("Clip not found".to_string()),
    }
}

#[tauri::command]
pub async fn move_to_folder(
    clip_id: String,
    folder_id: Option<String>,
    state: State<'static, AppState>,
) -> Result<(), String> {
    let inner = state.inner.lock().map_err(|e| e.to_string())?;
    let db = &inner.database;

    let folder_id = match folder_id {
        Some(id) => Some(id.parse::<i64>().map_err(|_| "Invalid folder ID")?),
        None => None,
    };

    let clip = futures::executor::block_on(async {
        db.get_clip_by_uuid(&clip_id).await
    }).map_err(|e| e.to_string())?;

    match clip {
        Some(clip) => {
            futures::executor::block_on(async {
                db.update_clip_folder(clip.id, folder_id).await.map_err(|e| e.to_string())?;
                Ok::<(), String>(())
            })?;
            Ok(())
        }
        None => Err("Clip not found".to_string()),
    }
}

#[tauri::command]
pub async fn create_folder(
    name: String,
    icon: Option<String>,
    color: Option<String>,
    state: State<'static, AppState>,
) -> Result<FolderItem, String> {
    let inner = state.inner.lock().map_err(|e| e.to_string())?;
    let db = &inner.database;

    let folder_id = futures::executor::block_on(async {
        db.create_folder(&name, icon.as_deref(), color.as_deref()).await
    }).map_err(|e| e.to_string())?;

    Ok(FolderItem {
        id: folder_id.to_string(),
        name,
        icon,
        color,
        is_system: false,
        item_count: 0,
    })
}

#[tauri::command]
pub async fn delete_folder(
    id: String,
    state: State<'static, AppState>,
) -> Result<(), String> {
    let inner = state.inner.lock().map_err(|e| e.to_string())?;
    let db = &inner.database;

    let folder_id = id.parse::<i64>().map_err(|_| "Invalid folder ID")?;

    futures::executor::block_on(async {
        db.delete_folder(folder_id).await.map_err(|e| e.to_string())?;
        Ok::<(), String>(())
    })?;

    Ok(())
}

#[tauri::command]
pub async fn search_clips(
    query: String,
    limit: i64,
    state: State<'static, AppState>,
) -> Result<Vec<ClipboardItem>, String> {
    let inner = state.inner.lock().map_err(|e| e.to_string())?;
    let db = &inner.database;

    let clips = futures::executor::block_on(async {
        db.search_clips(&query, limit).await
    }).map_err(|e| e.to_string())?;

    let items: Vec<ClipboardItem> = clips.iter().map(|clip| {
        let content_str = String::from_utf8_lossy(&clip.content).to_string();

        ClipboardItem {
            id: clip.uuid.clone(),
            clip_type: clip.clip_type.clone(),
            content: content_str.clone(),
            preview: clip.text_preview.clone(),
            is_pinned: clip.is_pinned,
            folder_id: clip.folder_id.map(|id| id.to_string()),
            created_at: clip.created_at.to_rfc3339(),
            source_app: clip.source_app.clone(),
        }
    }).collect();

    Ok(items)
}

#[tauri::command]
pub async fn get_folders(
    state: State<'static, AppState>,
) -> Result<Vec<FolderItem>, String> {
    let inner = state.inner.lock().map_err(|e| e.to_string())?;
    let db = &inner.database;

    let folders = futures::executor::block_on(async {
        db.get_folders().await
    }).map_err(|e| e.to_string())?;

    let items: Vec<FolderItem> = folders.iter().map(|folder| {
        FolderItem {
            id: folder.id.to_string(),
            name: folder.name.clone(),
            icon: folder.icon.clone(),
            color: folder.color.clone(),
            is_system: folder.is_system,
            item_count: 0,
        }
    }).collect();

    Ok(items)
}

#[tauri::command]
pub async fn get_settings(
    state: State<'static, AppState>,
) -> Result<super::models::Settings, String> {
    let inner = state.inner.lock().map_err(|e| e.to_string())?;
    let db = &inner.database;

    let mut settings = super::models::Settings::default();

    if let Ok(Some(value)) = futures::executor::block_on(async {
        db.get_setting("max_items").await
    }) {
        settings.max_items = value.parse().unwrap_or(1000);
    }

    if let Ok(Some(value)) = futures::executor::block_on(async {
        db.get_setting("auto_delete_days").await
    }) {
        settings.auto_delete_days = value.parse().unwrap_or(30);
    }

    if let Ok(Some(value)) = futures::executor::block_on(async {
        db.get_setting("theme").await
    }) {
        settings.theme = value;
    }

    Ok(settings)
}

#[tauri::command]
pub async fn save_settings(
    settings: super::models::Settings,
    state: State<'static, AppState>,
) -> Result<(), String> {
    let inner = state.inner.lock().map_err(|e| e.to_string())?;
    let db = &inner.database;

    futures::executor::block_on(async {
        db.set_setting("max_items", &settings.max_items.to_string()).await.map_err(|e| e.to_string())?;
        db.set_setting("auto_delete_days", &settings.auto_delete_days.to_string()).await.map_err(|e| e.to_string())?;
        db.set_setting("theme", &settings.theme).await.map_err(|e| e.to_string())?;
        Ok::<(), String>(())
    })?;

    Ok(())
}

#[tauri::command]
pub async fn hide_window(window: tauri::Window) -> Result<(), String> {
    window.hide().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_clipboard_history_size(
    state: State<'static, AppState>,
) -> Result<i64, String> {
    let inner = state.inner.lock().map_err(|e| e.to_string())?;
    let db = &inner.database;

    let size = futures::executor::block_on(async {
        db.get_clipboard_history_size().await
    }).map_err(|e| e.to_string())?;

    Ok(size)
}

#[tauri::command]
pub async fn clear_clipboard_history(
    state: State<'static, AppState>,
) -> Result<(), String> {
    let inner = state.inner.lock().map_err(|e| e.to_string())?;
    let db = &inner.database;

    futures::executor::block_on(async {
        db.clear_history().await.map_err(|e| e.to_string())?;
        Ok::<(), String>(())
    })?;

    Ok(())
}
