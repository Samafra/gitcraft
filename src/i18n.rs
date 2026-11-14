use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    Es,
    En,
}

impl Language {
    pub fn all() -> &'static [Language] {
        &[Language::Es, Language::En]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Language::Es => "Español",
            Language::En => "English",
        }
    }
}

pub struct I18nManager {
    bundles: HashMap<Language, HashMap<String, String>>,
}

impl Default for I18nManager {
    fn default() -> Self {
        let mut bundles = HashMap::new();
        let es_map: HashMap<String, String> =
            serde_json::from_str(include_str!("../locales/es.json")).unwrap_or_default();
        let en_map: HashMap<String, String> =
            serde_json::from_str(include_str!("../locales/en.json")).unwrap_or_default();
        bundles.insert(Language::Es, es_map);
        bundles.insert(Language::En, en_map);
        Self { bundles }
    }
}

impl I18nManager {
    pub fn t<'a>(&'a self, lang: Language, key: &'a str) -> &'a str {
        self.bundles
            .get(&lang)
            .and_then(|m| m.get(key))
            .or_else(|| self.bundles.get(&Language::En).and_then(|m| m.get(key)))
            .map(|s| s.as_str())
            .unwrap_or(key)
    }
}
