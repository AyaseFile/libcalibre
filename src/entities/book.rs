use chrono::{DateTime, Utc};
use diesel::prelude::*;

use serde::Deserialize;

use crate::schema::books;

#[derive(Clone, Debug, Queryable, Selectable, Identifiable, AsChangeset)]
#[diesel(table_name = books)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Book {
    pub id: i32,
    pub uuid: Option<String>,
    pub title: String,
    pub sort: Option<String>,
    pub timestamp: Option<DateTime<Utc>>,
    pub pubdate: Option<DateTime<Utc>>,
    pub series_index: f32,
    pub author_sort: Option<String>,
    pub path: String,
    pub flags: i32,
    pub has_cover: Option<bool>,
    pub last_modified: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = books)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct NewBook {
    pub title: String,
    pub timestamp: Option<DateTime<Utc>>,
    pub pubdate: Option<DateTime<Utc>>,
    pub series_index: f32,
    pub flags: i32,
    pub has_cover: Option<bool>,
}

#[derive(Deserialize, AsChangeset, Default, Debug)]
#[diesel(table_name = books)]
pub struct UpdateBookData {
    pub(crate) author_sort: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) timestamp: Option<DateTime<Utc>>,
    pub(crate) pubdate: Option<DateTime<Utc>>,
    pub(crate) series_index: Option<f32>,
    pub(crate) path: Option<String>,
    pub(crate) flags: Option<i32>,
    pub(crate) has_cover: Option<bool>,
    pub(crate) last_modified: Option<DateTime<Utc>>,
}

#[derive(Deserialize, Default, Debug)]
pub struct UpsertBookIdentifier {
    pub book_id: i32,
    pub id: Option<i32>,
    pub label: String,
    pub value: String,
}
