use isolang::Language;
use std::path::Path;

#[derive(Clone)]
pub struct ValidDbPath {
    pub(crate) library_path: String,
    pub(crate) database_path: String,
}

/// For a given library root root directory, return the path to the SQLite
/// database if the file is accessible.
pub fn get_db_path(library_root: &str) -> Option<ValidDbPath> {
    let db_path = Path::new(library_root).join("metadata.db");
    if db_path.exists() {
        Some(ValidDbPath {
            database_path: db_path.to_str().map(|s| s.to_string())?,
            library_path: library_root.to_string(),
        })
    } else {
        None
    }
}

pub fn canonicalize_lang(raw: &str) -> Option<Language> {
    let raw = raw.trim().to_lowercase();
    if raw.is_empty() {
        return None;
    }

    if let Some(lang) = Language::from_name_case_insensitive(&raw) {
        return Some(lang);
    }

    match raw.len() {
        2 => Language::from_639_1(&raw),
        3 => Language::from_639_3(&raw),
        _ => None,
    }
}

trait LanguageExt {
    fn from_name_case_insensitive(name: &str) -> Option<Language>;
}

impl LanguageExt for Language {
    fn from_name_case_insensitive(name: &str) -> Option<Language> {
        Language::from_name(name)
            // if failed, try to capitalize the first letter
            .or_else(|| {
                if let Some(first_char) = name.chars().next() {
                    let capitalized = first_char.to_uppercase().collect::<String>()
                        + &name[first_char.len_utf8()..];
                    Language::from_name(&capitalized)
                } else {
                    None
                }
            })
    }
}
