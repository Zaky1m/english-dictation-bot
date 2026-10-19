use crate::exercise::scoring::Params;
use serde_json::Value;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::RwLock;

/// In-memory copy of the `settings` table. Rebuildable from the database, so
/// it is only a cache; saving a setting reloads it immediately.
pub struct Settings {
    map: RwLock<HashMap<String, Value>>,
}

impl Settings {
    #[cfg(test)]
    pub fn empty() -> Self {
        Self {
            map: RwLock::new(HashMap::new()),
        }
    }

    pub async fn load(pool: &PgPool) -> Result<Self, sqlx::Error> {
        let settings = Self {
            map: RwLock::new(HashMap::new()),
        };
        settings.reload(pool).await?;
        Ok(settings)
    }

    pub async fn reload(&self, pool: &PgPool) -> Result<(), sqlx::Error> {
        let rows: Vec<(String, Value)> = sqlx::query_as("SELECT key, value FROM settings")
            .fetch_all(pool)
            .await?;
        *self.map.write().unwrap_or_else(|p| p.into_inner()) = rows.into_iter().collect();
        Ok(())
    }

    fn get(&self, key: &str) -> Option<Value> {
        self.map
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .get(key)
            .cloned()
    }

    pub fn int(&self, key: &str) -> i64 {
        self.get(key).and_then(|v| v.as_i64()).unwrap_or_else(|| {
            tracing::error!(key, "integer setting missing or invalid");
            0
        })
    }

    pub fn float(&self, key: &str) -> f64 {
        self.get(key).and_then(|v| v.as_f64()).unwrap_or_else(|| {
            tracing::error!(key, "numeric setting missing or invalid");
            0.0
        })
    }

    pub fn bool(&self, key: &str) -> bool {
        self.get(key).and_then(|v| v.as_bool()).unwrap_or(false)
    }

    pub fn int_list(&self, key: &str) -> Vec<i64> {
        self.get(key)
            .and_then(|v| v.as_array().cloned())
            .map(|a| a.iter().filter_map(Value::as_i64).collect())
            .unwrap_or_default()
    }

    pub fn score_params(&self) -> Params {
        Params {
            base_constant: self.float("score_base_constant"),
            mistake_penalty: self.float("score_mistake_penalty"),
            reveal_penalty: self.float("score_reveal_penalty"),
            min_accuracy: self.float("score_min_accuracy"),
            expected_base_ms: self.float("score_expected_base_ms"),
            expected_per_letter_ms: self.float("score_expected_per_letter_ms"),
            speed_bonus_factor: self.float("score_speed_bonus_factor"),
            combo_step: self.float("score_combo_step"),
            combo_cap: self.int("score_combo_cap").max(0) as u32,
            min_ms_per_letter: self.float("score_min_ms_per_letter"),
        }
    }
}
