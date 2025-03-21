use diesel::prelude::*;
use diesel::query_builder::AsChangeset;
use serde::Deserialize;

use crate::schema::publishers;

#[derive(Clone, Debug, Queryable, Selectable, Identifiable, AsChangeset)]
#[diesel(table_name = publishers)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Publisher {
    pub id: i32,
    pub name: String,
    pub sort: Option<String>,
}

#[derive(Insertable)]
#[diesel(table_name = publishers)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct NewPublisher {
    pub name: String,
    pub sort: Option<String>,
}

#[derive(Deserialize, AsChangeset, Default, Debug)]
#[diesel(table_name = publishers)]
pub struct UpdatePublisherData {
    pub(crate) name: Option<String>,
    pub(crate) sort: Option<String>,
}
