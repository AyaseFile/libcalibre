use diesel::prelude::*;
use diesel::query_builder::AsChangeset;
use serde::Deserialize;

use crate::schema::ratings;

#[derive(Clone, Debug, Queryable, Selectable, Identifiable, AsChangeset)]
#[diesel(table_name = ratings)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Rating {
    pub id: i32,
    pub rating: i32,
}

#[derive(Insertable)]
#[diesel(table_name = ratings)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct NewRating {
    pub rating: i32,
}

#[derive(Deserialize, AsChangeset, Default, Debug)]
#[diesel(table_name = ratings)]
pub struct UpdateRatingData {
    pub(crate) rating: Option<i32>,
}
