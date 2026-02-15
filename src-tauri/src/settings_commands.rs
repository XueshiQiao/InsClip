use tauri::{AppHandle, Manager};
use crate::settings_manager::SettingsManager;
use dark_light::Mode;
use std::sync::Arc;

#[tauri::command]
pub async fn get_settings(app: AppHandle) -> Result<serde_json::Value, String> {
    let manager = app.state::<Arc<SettingsManager>>();
    let settings = manager.get();
    let mut value = serde_json::to_value(&settings).map_err(|e| e.to_string())?;

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
pub async fn save_settings(app: AppHandle, settings: serde_json::Value) -> Result<(), String> {
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
