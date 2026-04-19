use std::path::PathBuf;

use crate::core::json::JsonValue;

/// Paths resolved using cascading lookup:
///   1. `.viv/settings.json` in the current working directory (highest priority)
///   2. `~/.viv/settings.json` in the home directory (fallback)
#[derive(Debug, Clone)]
pub struct ConfigPaths {
    pub settings: PathBuf,
    pub skills_project: PathBuf,
    pub skills_user: PathBuf,
}

impl Default for ConfigPaths {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigPaths {
    pub fn new() -> Self {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));

        ConfigPaths {
            settings: PathBuf::from(".viv/settings.json"),
            skills_project: PathBuf::from(".viv/skills"),
            skills_user: home.join(".viv/skills"),
        }
    }

    /// Returns the path to `settings.json` if it exists in the current working
    /// directory. Falls back to `~/.viv/settings.json` if the local one is
    /// absent. Returns `None` if neither file exists.
    pub fn settings_path(&self) -> Option<PathBuf> {
        if self.settings.exists() {
            return Some(self.settings.clone());
        }
        let user_settings = self.skills_user.parent()?.join("settings.json");
        if user_settings.exists() {
            return Some(user_settings);
        }
        None
    }

    /// Returns the skills directories in priority order: project first, then
    /// user (mirrors the cascade used for settings).
    pub fn skills_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = Vec::with_capacity(2);
        if self.skills_project.exists() {
            dirs.push(self.skills_project.clone());
        }
        if self.skills_user.exists() {
            dirs.push(self.skills_user.clone());
        }
        dirs
    }

    /// Load and merge model configuration from cascading settings files.
    ///
    /// Priority (each field independently): project → user → empty (env var fallback
    /// is handled by `LLMConfig::from_env`).
    pub fn model_config(&self) -> crate::Result<ModelConfig> {
        // Load user config from ~/.viv/settings.json
        let user_config = self.load_model_from_path(&self.skills_user.parent().map(|p| p.join("settings.json")).unwrap_or_default());

        // Load project config from ./.viv/settings.json (overrides user)
        let project_config = self.load_model_from_path(&self.settings);

        Ok(project_config.merge(&user_config))
    }

    fn load_model_from_path(&self, path: &std::path::Path) -> ModelConfig {
        let contents = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => return ModelConfig::default(),
        };
        match ModelConfig::parse(&contents) {
            Ok(c) => c,
            Err(_) => ModelConfig::default(),
        }
    }
}

/// Model configuration parsed from settings.json.
///
/// All fields are optional — missing fields fall back to environment variables
/// or built-in defaults. Lookup priority (for each field independently):
///   1. Project `.viv/settings.json`
///   2. User `~/.viv/settings.json`
///   3. Environment variables
#[derive(Debug, Clone, Default)]
pub struct ModelConfig {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model_fast: Option<String>,
    pub model_medium: Option<String>,
    pub model_slow: Option<String>,
}

impl ModelConfig {
    /// Load model config from a JSON string.
    ///
    /// Returns an empty config if the string is empty or has no `model` field.
    pub fn parse(json_str: &str) -> crate::Result<Self> {
        if json_str.trim().is_empty() {
            return Ok(ModelConfig::default());
        }
        let root = JsonValue::parse(json_str)?;

        let model_obj = match root.get("model") {
            Some(v) => v,
            None => return Ok(ModelConfig::default()),
        };

        let obj = model_obj.as_object().ok_or_else(|| {
            crate::Error::Json("'model' must be an object".to_string())
        })?;

        let get_str = |obj: &std::vec::Vec<(String, JsonValue)>, key: &str| {
            obj.iter().find(|(k, _)| k == key).and_then(|(_, v)| v.as_str()).map(|s| s.to_string())
        };

        Ok(ModelConfig {
            api_key: get_str(obj, "apiKey"),
            base_url: get_str(obj, "baseUrl"),
            model_fast: get_str(obj, "fast"),
            model_medium: get_str(obj, "medium"),
            model_slow: get_str(obj, "slow"),
        })
    }

    /// Merge two configs: `self` overrides `fallback`.
    ///
    /// Used to implement cascading: project config overrides user config.
    pub fn merge(&self, fallback: &ModelConfig) -> ModelConfig {
        ModelConfig {
            api_key: self.api_key.clone().or_else(|| fallback.api_key.clone()),
            base_url: self.base_url.clone().or_else(|| fallback.base_url.clone()),
            model_fast: self.model_fast.clone().or_else(|| fallback.model_fast.clone()),
            model_medium: self.model_medium.clone().or_else(|| fallback.model_medium.clone()),
            model_slow: self.model_slow.clone().or_else(|| fallback.model_slow.clone()),
        }
    }
}
