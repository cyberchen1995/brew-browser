//! Categories command. Returns the bundled `categories.json` (LLM-generated
//! offline by `tools/categorize/`) so the frontend can render category tiles
//! and filter chips without round-tripping to disk.
//!
//! The JSON is embedded into the binary at compile time via `include_str!`,
//! so there is no runtime file dependency and no disk read on every call.
//! Parsing happens once per process and is memoised on `AppState`.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::BrewError;
use crate::state::AppState;

const CATEGORIES_JSON: &str = include_str!("../../data/categories.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryMeta {
    pub label: String,
    pub icon: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoriesData {
    pub version: String,
    /// Bundled `categories.json` uses snake_case; IPC wire format is camelCase.
    /// `alias` keeps the deserializer happy with the on-disk shape; the field
    /// serializes as `generatedAt` over IPC per the struct's `rename_all` rule.
    #[serde(alias = "generated_at")]
    pub generated_at: String,
    pub model: String,
    pub categories: BTreeMap<String, CategoryMeta>,
    pub casks: BTreeMap<String, Vec<String>>,
    pub formulae: BTreeMap<String, Vec<String>>,
}

#[tauri::command]
pub async fn categories_data(state: State<'_, AppState>) -> Result<Arc<CategoriesData>, BrewError> {
    {
        let cached = state.categories_cache.lock().await;
        if let Some(data) = cached.as_ref() {
            return Ok(Arc::clone(data));
        }
    }

    let parsed: CategoriesData =
        serde_json::from_str(CATEGORIES_JSON).map_err(|e| BrewError::Internal {
            message: format!("categories.json parse failed: {e}"),
        })?;
    let arc = Arc::new(parsed);

    let mut cached = state.categories_cache.lock().await;
    *cached = Some(Arc::clone(&arc));
    Ok(arc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_json_parses() {
        let parsed: CategoriesData =
            serde_json::from_str(CATEGORIES_JSON).expect("bundled categories.json must parse");
        assert!(
            parsed.categories.len() >= 15,
            "expected at least 15 category slugs"
        );
        assert!(parsed.casks.len() > 1000, "expected >1k casks");
        assert!(parsed.formulae.len() > 1000, "expected >1k formulae");
        // every item's category slug must be present in the categories map
        for (token, cats) in parsed.casks.iter().chain(parsed.formulae.iter()) {
            for c in cats {
                assert!(
                    parsed.categories.contains_key(c),
                    "{token} references unknown category {c}"
                );
            }
        }
    }
}
