use diesel::prelude::*;
use diesel::query_builder::AsChangeset;
use serde::Deserialize;

use crate::schema::tags;

#[derive(Clone, Debug, Queryable, Selectable, Identifiable, AsChangeset)]
#[diesel(table_name = tags)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Tag {
    pub id: i32,
    pub name: String,
}

#[derive(Insertable)]
#[diesel(table_name = tags)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct NewTag {
    pub name: String,
}

#[derive(Deserialize, AsChangeset, Default, Debug)]
#[diesel(table_name = tags)]
pub struct UpdateTagData {
    pub(crate) name: Option<String>,
}
