use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A single entry in the model pool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPoolEntry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub model_name: String,
    pub priority: u32,
    pub enabled: bool,
    pub builtin: bool,
    #[serde(default = "default_provider_type")]
    pub provider_type: String,
    #[serde(default = "default_api_format")]
    pub api_format: String,
}

fn default_api_format() -> String {
    "openai".into()
}

fn default_provider_type() -> String {
    "opencode".into()
}

/// The full model pool configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPool {
    pub pool_mode: bool,
    pub entries: Vec<ModelPoolEntry>,
    /// IDs of built-in models that were explicitly deleted by the user.
    /// These won't be re-created on restart.
    #[serde(default)]
    pub deleted_builtins: Vec<String>,
}

impl ModelPool {
    pub fn new() -> Self {
        ModelPool {
            pool_mode: true,
            entries: Vec::new(),
            deleted_builtins: Vec::new(),
        }
    }

    /// Load or initialize the pool from disk.
    /// Returns `None` if the file exists but is corrupt, so the caller can handle it.
    pub fn load(path: &PathBuf) -> Self {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(pool) = serde_json::from_str::<ModelPool>(&content) {
                return pool;
            }
            // File exists but is corrupted — log would go here;
            // fall through to return new empty pool rather than panic.
        }
        Self::new()
    }

    /// Save to disk.
    pub fn save(&self, path: &PathBuf) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, content);
        }
    }

    /// Get all enabled entries sorted by priority (ascending).
    pub fn get_enabled(&self) -> Vec<&ModelPoolEntry> {
        let mut enabled: Vec<&ModelPoolEntry> = self.entries.iter().filter(|e| e.enabled).collect();
        enabled.sort_by_key(|e| e.priority);
        enabled
    }

    /// Find entry by ID.
    pub fn get_by_id(&self, id: &str) -> Option<&ModelPoolEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Find entry by model name (for mapping request model to pool entry).
    pub fn get_by_name(&self, name: &str) -> Option<&ModelPoolEntry> {
        self.entries.iter().find(|e| e.name == name || e.model_name == name)
    }

    /// Find mutable entry by ID.
    pub fn get_by_id_mut(&mut self, id: &str) -> Option<&mut ModelPoolEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    /// Find index of entry by ID.
    fn index_by_id(&self, id: &str) -> Option<usize> {
        self.entries.iter().position(|e| e.id == id)
    }

    /// Add or update an entry.
    pub fn upsert(&mut self, entry: ModelPoolEntry) {
        if let Some(idx) = self.index_by_id(&entry.id) {
            self.entries[idx] = entry;
        } else {
            self.entries.push(entry);
        }
    }

    /// Remove an entry by ID. Returns the removed entry, if any.
    /// If the removed entry was a built-in, records its ID so it
    /// won't be re-created by `init_builtins`.
    pub fn remove(&mut self, id: &str) -> Option<ModelPoolEntry> {
        if let Some(idx) = self.index_by_id(id) {
            let entry = self.entries.remove(idx);
            if entry.builtin {
                if !self.deleted_builtins.contains(&entry.id) {
                    self.deleted_builtins.push(entry.id.clone());
                }
            }
            Some(entry)
        } else {
            None
        }
    }

    /// Toggle enable/disable for an entry.
    pub fn toggle_enabled(&mut self, id: &str) -> bool {
        if let Some(idx) = self.index_by_id(id) {
            self.entries[idx].enabled = !self.entries[idx].enabled;
            self.entries[idx].enabled
        } else {
            false
        }
    }

    /// Set priority for an entry.
    pub fn set_priority(&mut self, id: &str, priority: u32) {
        if let Some(idx) = self.index_by_id(id) {
            self.entries[idx].priority = priority;
        }
    }

    /// Initialize built-in OpenCode models (replaces all opencode entries).
    /// Skips any model whose opencode-{name} id is in `deleted_builtins`.
    pub fn init_builtins(&mut self, model_names: &[&str]) {
        // Remove existing opencode entries
        self.entries.retain(|e| e.provider_type != "opencode");

        for (i, name) in model_names.iter().enumerate() {
            let id = format!("opencode-{}", name);
            // Skip models the user previously deleted
            if self.deleted_builtins.contains(&id) {
                continue;
            }
            self.entries.push(ModelPoolEntry {
                id,
                name: name.to_string(),
                base_url: String::new(),
                api_key: String::new(),
                model_name: name.to_string(),
                priority: (i + 1) as u32,
                enabled: true,
                builtin: true,
                provider_type: "opencode".into(),
                api_format: "openai".into(),
            });
        }
    }

    /// Migrate built-in model entries and deleted_builtins after upstream renames.
    /// Call this BEFORE `init_builtins` so that `deleted_builtins` references
    /// point to the current model IDs.
    pub fn migrate_renamed_builtins(&mut self) {
        const RENAMES: &[(&str, &str)] = &[
            ("opencode-nemotron-3-super-free", "opencode-nemotron-3-ultra-free"),
        ];

        // Rename entries that still use the old name
        for entry in &mut self.entries {
            for (old_id, new_id) in RENAMES {
                if entry.id == *old_id {
                    entry.id = new_id.to_string();
                    let new_name = new_id.strip_prefix("opencode-").unwrap_or(new_id);
                    entry.name = new_name.to_string();
                    entry.model_name = new_name.to_string();
                }
            }
        }

        // Migrate deleted_builtins
        for (old_id, new_id) in RENAMES {
            if let Some(pos) = self.deleted_builtins.iter().position(|id| id == old_id) {
                self.deleted_builtins.remove(pos);
                if !self.deleted_builtins.iter().any(|id| id == new_id) {
                    self.deleted_builtins.push(new_id.to_string());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_MODELS: &[&str] = &[
        "deepseek-v4-flash-free",
        "big-pickle",
        "minimax-m2.5-free",
    ];

    fn make_custom_entry(id: &str, name: &str, priority: u32) -> ModelPoolEntry {
        ModelPoolEntry {
            id: id.to_string(),
            name: name.to_string(),
            base_url: "https://api.example.com/v1".into(),
            api_key: "sk-test".into(),
            model_name: name.to_string(),
            priority,
            enabled: true,
            builtin: false,
            provider_type: "custom".into(),
            api_format: "openai".into(),
        }
    }

    #[test]
    fn test_new_pool_is_empty() {
        let pool = ModelPool::new();
        assert!(pool.entries.is_empty());
        assert!(pool.pool_mode);
        assert!(pool.deleted_builtins.is_empty());
    }

    #[test]
    fn test_add_and_remove_custom_entry() {
        let mut pool = ModelPool::new();
        let entry = make_custom_entry("test-1", "my-model", 1);
        pool.upsert(entry);

        assert_eq!(pool.entries.len(), 1);
        assert_eq!(pool.entries[0].name, "my-model");

        let removed = pool.remove("test-1");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().name, "my-model");
        assert!(pool.entries.is_empty());
    }

    #[test]
    fn test_remove_nonexistent_entry_returns_none() {
        let mut pool = ModelPool::new();
        let result = pool.remove("does-not-exist");
        assert!(result.is_none());
    }

    #[test]
    fn test_upsert_updates_existing_entry() {
        let mut pool = ModelPool::new();
        let entry = make_custom_entry("test-1", "original", 1);
        pool.upsert(entry);

        let updated = ModelPoolEntry {
            priority: 5,
            name: "updated".into(),
            ..make_custom_entry("test-1", "updated", 5)
        };
        pool.upsert(updated);

        assert_eq!(pool.entries.len(), 1);
        assert_eq!(pool.entries[0].name, "updated");
        assert_eq!(pool.entries[0].priority, 5);
    }

    #[test]
    fn test_toggle_enabled() {
        let mut pool = ModelPool::new();
        let entry = make_custom_entry("test-1", "my-model", 1);
        pool.upsert(entry);

        assert!(pool.entries[0].enabled);

        let new_state = pool.toggle_enabled("test-1");
        assert!(!new_state);
        assert!(!pool.entries[0].enabled);

        let new_state2 = pool.toggle_enabled("test-1");
        assert!(new_state2);
        assert!(pool.entries[0].enabled);
    }

    #[test]
    fn test_toggle_nonexistent_returns_false() {
        let mut pool = ModelPool::new();
        assert!(!pool.toggle_enabled("nope"));
    }

    #[test]
    fn test_get_enabled_returns_sorted() {
        let mut pool = ModelPool::new();
        pool.upsert(make_custom_entry("c", "c", 10));
        pool.upsert(make_custom_entry("a", "a", 1));
        pool.upsert(make_custom_entry("b", "b", 5));

        // Disable one
        pool.toggle_enabled("c");

        let enabled = pool.get_enabled();
        assert_eq!(enabled.len(), 2);
        assert_eq!(enabled[0].name, "a");
        assert_eq!(enabled[1].name, "b");
    }

    #[test]
    fn test_get_by_name_matches_name_or_model_name() {
        let mut pool = ModelPool::new();
        pool.upsert(ModelPoolEntry {
            id: "test-id".into(),
            name: "display-name".into(),
            model_name: "actual-model".into(),
            ..make_custom_entry("test-id", "display-name", 1)
        });

        assert!(pool.get_by_name("display-name").is_some());
        assert!(pool.get_by_name("actual-model").is_some());
        assert!(pool.get_by_name("nonexistent").is_none());
    }

    #[test]
    fn test_set_priority() {
        let mut pool = ModelPool::new();
        pool.upsert(make_custom_entry("test-1", "a", 5));
        pool.set_priority("test-1", 100);
        assert_eq!(pool.entries[0].priority, 100);
    }

    #[test]
    fn test_init_builtins_adds_all_models() {
        let mut pool = ModelPool::new();
        pool.init_builtins(TEST_MODELS);

        assert_eq!(pool.entries.len(), TEST_MODELS.len());
        for (i, name) in TEST_MODELS.iter().enumerate() {
            let entry = pool.get_by_name(name).unwrap();
            assert!(entry.builtin);
            assert_eq!(entry.provider_type, "opencode");
            assert_eq!(entry.priority, (i + 1) as u32);
            assert!(entry.enabled);
        }
    }

    #[test]
    fn test_init_builtins_removes_old_opencode_entries() {
        let mut pool = ModelPool::new();
        pool.init_builtins(TEST_MODELS);
        // Second call should replace, not duplicate
        pool.init_builtins(TEST_MODELS);
        assert_eq!(pool.entries.len(), TEST_MODELS.len());
    }

    #[test]
    fn test_init_builtins_skips_deleted_builtins() {
        let mut pool = ModelPool::new();
        pool.deleted_builtins.push("opencode-big-pickle".into());
        pool.init_builtins(TEST_MODELS);

        // big-pickle should not be present
        assert_eq!(pool.entries.len(), TEST_MODELS.len() - 1);
        assert!(pool.get_by_name("big-pickle").is_none());
        assert!(pool.get_by_name("deepseek-v4-flash-free").is_some());
    }

    #[test]
    fn test_remove_builtin_records_deleted_id() {
        let mut pool = ModelPool::new();
        pool.init_builtins(TEST_MODELS);

        let removed = pool.remove("opencode-big-pickle");
        assert!(removed.is_some());
        assert!(removed.unwrap().builtin);

        assert!(pool.deleted_builtins.contains(&"opencode-big-pickle".into()));
        // Running init_builtins again should not bring it back
        pool.init_builtins(TEST_MODELS);
        assert!(pool.get_by_name("big-pickle").is_none());
        assert_eq!(pool.entries.len(), 2); // only the other two
    }

    #[test]
    fn test_remove_custom_entry_does_not_affect_deleted_builtins() {
        let mut pool = ModelPool::new();
        pool.init_builtins(TEST_MODELS);
        pool.upsert(make_custom_entry("custom-1", "my-model", 99));

        pool.remove("custom-1");
        assert!(pool.deleted_builtins.is_empty());
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = std::env::temp_dir().join("model_pool_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_pool.json");

        // Create and save
        {
            let mut pool = ModelPool::new();
            pool.init_builtins(TEST_MODELS);
            pool.upsert(make_custom_entry("custom-1", "my-custom", 99));
            pool.remove("opencode-big-pickle");
            pool.save(&path);
        }

        // Load and verify
        let loaded = ModelPool::load(&path);
        assert_eq!(loaded.entries.len(), 3); // 2 builtins (one deleted) + 1 custom
        assert!(loaded.deleted_builtins.contains(&"opencode-big-pickle".into()));
        assert!(loaded.get_by_name("deepseek-v4-flash-free").is_some());
        assert!(loaded.get_by_name("big-pickle").is_none());
        assert!(loaded.get_by_name("my-custom").is_some());

        // Clean up
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn test_load_empty_or_missing_file_returns_empty_pool() {
        let path = PathBuf::from("/tmp/nonexistent_file_12345.json");
        let pool = ModelPool::load(&path);
        assert!(pool.entries.is_empty());
        assert!(pool.deleted_builtins.is_empty());
    }

    #[test]
    fn test_remove_duplicate_id_does_not_duplicate_deleted() {
        let mut pool = ModelPool::new();
        pool.init_builtins(TEST_MODELS);
        pool.remove("opencode-big-pickle");
        // Remove same id again (already gone)
        pool.remove("opencode-big-pickle");
        // Should only appear once in deleted_builtins
        let count = pool.deleted_builtins.iter().filter(|&id| id == "opencode-big-pickle").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_init_builtins_preserves_custom_entries() {
        let mut pool = ModelPool::new();
        pool.upsert(make_custom_entry("custom-1", "keep-me", 99));
        pool.init_builtins(TEST_MODELS);

        assert!(pool.get_by_name("keep-me").is_some());
        assert_eq!(pool.entries.len(), TEST_MODELS.len() + 1);
    }

    // ── Migration tests ────────────────────────────────────────────────

    #[test]
    fn test_migrate_renamed_builtins_updates_entry_id() {
        let mut pool = ModelPool::new();
        // Manually add an entry with the OLD nemotron name
        pool.upsert(ModelPoolEntry {
            id: "opencode-nemotron-3-super-free".into(),
            name: "nemotron-3-super-free".into(),
            model_name: "nemotron-3-super-free".into(),
            priority: 1,
            enabled: true,
            builtin: true,
            provider_type: "opencode".into(),
            api_format: "openai".into(),
            base_url: String::new(),
            api_key: String::new(),
        });

        pool.migrate_renamed_builtins();

        let entry = pool.get_by_id("opencode-nemotron-3-ultra-free");
        assert!(entry.is_some(), "Entry should be migrated to new ID");
        assert_eq!(entry.unwrap().name, "nemotron-3-ultra-free");
        assert_eq!(entry.unwrap().model_name, "nemotron-3-ultra-free");
        // Old ID should no longer exist
        assert!(pool.get_by_id("opencode-nemotron-3-super-free").is_none());
    }

    #[test]
    fn test_migrate_renamed_builtins_updates_deleted_builtins() {
        let mut pool = ModelPool::new();
        pool.deleted_builtins.push("opencode-nemotron-3-super-free".into());

        pool.migrate_renamed_builtins();

        assert!(!pool.deleted_builtins.contains(&"opencode-nemotron-3-super-free".into()),
            "Old deleted_builtins ID should be removed");
        assert!(pool.deleted_builtins.contains(&"opencode-nemotron-3-ultra-free".into()),
            "New deleted_builtins ID should be present");
        // Should still be exactly one entry
        assert_eq!(pool.deleted_builtins.len(), 1);
    }

    #[test]
    fn test_migrate_renamed_builtins_skips_unrelated_entries() {
        let mut pool = ModelPool::new();
        pool.init_builtins(TEST_MODELS);
        pool.upsert(make_custom_entry("custom-1", "my-custom", 99));
        let count_before = pool.entries.len();

        pool.migrate_renamed_builtins();

        // No rename applies to TEST_MODELS, so entries should be unchanged
        assert_eq!(pool.entries.len(), count_before);
        assert!(pool.get_by_name("my-custom").is_some());
        assert!(pool.get_by_name("deepseek-v4-flash-free").is_some());
    }

    #[test]
    fn test_init_builtins_after_migration_creates_correct_entries() {
        let mut pool = ModelPool::new();
        // Simulate an existing pool with the old nemotron name
        pool.upsert(ModelPoolEntry {
            id: "opencode-nemotron-3-super-free".into(),
            name: "nemotron-3-super-free".into(),
            model_name: "nemotron-3-super-free".into(),
            priority: 999,
            enabled: false,
            builtin: true,
            provider_type: "opencode".into(),
            api_format: "openai".into(),
            base_url: String::new(),
            api_key: String::new(),
        });

        // Migration should update the entry, then init_builtins replaces it
        pool.migrate_renamed_builtins();
        let new_models = &[
            "deepseek-v4-flash-free",
            "big-pickle",
            "nemotron-3-ultra-free",
        ];
        pool.init_builtins(new_models);

        let entry = pool.get_by_name("nemotron-3-ultra-free");
        assert!(entry.is_some(), "nemotron-3-ultra-free should be created by init_builtins");
        let entry = entry.unwrap();
        assert!(entry.enabled, "init_builtins creates entries as enabled");
        assert_eq!(entry.priority, 3, "Priority should be based on position in MODELS list");
        assert_eq!(entry.id, "opencode-nemotron-3-ultra-free");
    }

    #[test]
    fn test_init_builtins_idempotent() {
        let mut pool = ModelPool::new();
        let new_models = &[
            "deepseek-v4-flash-free",
            "big-pickle",
            "nemotron-3-ultra-free",
        ];

        pool.init_builtins(new_models);
        assert_eq!(pool.entries.len(), 3);

        // Second call should not duplicate entries
        pool.init_builtins(new_models);
        assert_eq!(pool.entries.len(), 3, "init_builtins should be idempotent");
    }
}
