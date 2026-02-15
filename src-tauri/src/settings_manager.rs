use crate::models::AppSettings;
use crate::database::Database;
use std::sync::RwLock;
use std::path::PathBuf;
use std::fs;
use tauri::AppHandle;
use tauri::Manager;

pub struct SettingsManager {
    file_path: PathBuf,
    settings: RwLock<AppSettings>,
}

impl SettingsManager {
    pub async fn new(app: &AppHandle, db: &Database) -> Self {
        let path = app.path().app_data_dir().unwrap().join("settings.json");
        let settings = if path.exists() {
            // Load from file
            let content = fs::read_to_string(&path).unwrap_or_default();
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            // Migrate from SQLite or use default
            Self::migrate_from_sqlite(db).await
        };

        // Ensure we save it once immediately if migrating, so file exists
        let manager = Self {
            file_path: path,
            settings: RwLock::new(settings.clone()),
        };
        if !manager.file_path.exists() {
            let _ = manager.save(settings);
        }
        manager
    }

    async fn migrate_from_sqlite(db: &Database) -> AppSettings {
        let mut settings = AppSettings::default();
        let pool = &db.pool;
        
        // Helper to fetch string
        // We can't capture pool easily in closure with async lifetime issues without boxing
        // So we just inline the queries or use a macro
        
        if let Ok(Some(v)) = sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 'theme'").fetch_optional(pool).await { settings.theme = v; }
        if let Ok(Some(v)) = sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 'mica_effect'").fetch_optional(pool).await { settings.mica_effect = v; }
        if let Ok(Some(v)) = sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 'language'").fetch_optional(pool).await { settings.language = v; }
        
        if let Ok(Some(v)) = sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 'max_items'").fetch_optional(pool).await { 
            if let Ok(i) = v.parse() { settings.max_items = i; }
        }
        if let Ok(Some(v)) = sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 'auto_delete_days'").fetch_optional(pool).await { 
            if let Ok(i) = v.parse() { settings.auto_delete_days = i; }
        }
        if let Ok(Some(v)) = sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 'hotkey'").fetch_optional(pool).await { settings.hotkey = v; }
        
        if let Ok(Some(v)) = sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 'auto_paste'").fetch_optional(pool).await { 
            if let Ok(b) = v.parse() { settings.auto_paste = b; }
        }
        if let Ok(Some(v)) = sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 'ignore_ghost_clips'").fetch_optional(pool).await { 
            if let Ok(b) = v.parse() { settings.ignore_ghost_clips = b; }
        }

        // AI
        if let Ok(Some(v)) = sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 'ai_provider'").fetch_optional(pool).await { settings.ai_provider = v; }
        if let Ok(Some(v)) = sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 'ai_api_key'").fetch_optional(pool).await { settings.ai_api_key = v; }
        if let Ok(Some(v)) = sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 'ai_model'").fetch_optional(pool).await { settings.ai_model = v; }
        if let Ok(Some(v)) = sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 'ai_base_url'").fetch_optional(pool).await { settings.ai_base_url = v; }
        
        if let Ok(Some(v)) = sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 'ai_prompt_summarize'").fetch_optional(pool).await { settings.ai_prompt_summarize = v; }
        if let Ok(Some(v)) = sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 'ai_prompt_translate'").fetch_optional(pool).await { settings.ai_prompt_translate = v; }
        if let Ok(Some(v)) = sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 'ai_prompt_explain_code'").fetch_optional(pool).await { settings.ai_prompt_explain_code = v; }
        if let Ok(Some(v)) = sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 'ai_prompt_fix_grammar'").fetch_optional(pool).await { settings.ai_prompt_fix_grammar = v; }
        
        if let Ok(Some(v)) = sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 'ai_title_summarize'").fetch_optional(pool).await { settings.ai_title_summarize = v; }
        if let Ok(Some(v)) = sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 'ai_title_translate'").fetch_optional(pool).await { settings.ai_title_translate = v; }
        if let Ok(Some(v)) = sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 'ai_title_explain_code'").fetch_optional(pool).await { settings.ai_title_explain_code = v; }
        if let Ok(Some(v)) = sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = 'ai_title_fix_grammar'").fetch_optional(pool).await { settings.ai_title_fix_grammar = v; }

        // Ignored Apps
        if let Ok(apps) = sqlx::query_scalar::<_, String>("SELECT app_name FROM ignored_apps").fetch_all(pool).await {
            settings.ignored_apps = apps.into_iter().collect();
        }

        settings
    }

    pub fn get(&self) -> AppSettings {
        self.settings.read().unwrap().clone()
    }

    pub fn save(&self, new_settings: AppSettings) -> Result<(), String> {
        {
            let mut lock = self.settings.write().unwrap();
            *lock = new_settings.clone();
        }
        
        let json = serde_json::to_string_pretty(&new_settings).map_err(|e| e.to_string())?;
        fs::write(&self.file_path, json).map_err(|e| e.to_string())?;
        
        Ok(())
    }
}
