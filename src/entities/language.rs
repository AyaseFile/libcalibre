use diesel::prelude::*;
use diesel::query_builder::AsChangeset;
use serde::Deserialize;

use crate::schema::languages;

#[derive(Clone, Debug, Queryable, Selectable, Identifiable, AsChangeset)]
#[diesel(table_name = languages)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Language {
    pub id: i32,
    pub lang_code: String,
}

#[derive(Insertable)]
#[diesel(table_name = languages)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct NewLanguage {
    pub lang_code: String,
}

#[derive(Deserialize, AsChangeset, Default, Debug)]
#[diesel(table_name = languages)]
pub struct UpdateLanguageData {
    pub(crate) lang_code: Option<String>,
}
