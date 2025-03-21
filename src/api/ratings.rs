use std::sync::Arc;
use std::sync::Mutex;

use diesel::prelude::*;

use crate::dtos::rating::NewRatingDto;
use crate::entities::rating::NewRating;
use crate::Rating;

pub struct RatingsHandler {
    client: Arc<Mutex<SqliteConnection>>,
}

impl RatingsHandler {
    pub(crate) fn new(client: Arc<Mutex<SqliteConnection>>) -> Self {
        Self { client }
    }

    pub fn create(&mut self, dto: NewRatingDto) -> Result<Rating, ()> {
        use crate::schema::ratings::dsl::ratings;
        let new_rating = NewRating::try_from(dto)?;
        let mut connection = self.client.lock().unwrap();

        diesel::insert_into(ratings)
            .values(new_rating)
            .returning(Rating::as_returning())
            .get_result::<Rating>(&mut *connection)
            .or(Err(()))
    }

    pub fn create_if_missing(&mut self, dto: NewRatingDto) -> Result<Rating, ()> {
        match self.find_by_value(dto.rating)? {
            Some(rating) => Ok(rating),
            _ => self.create(dto),
        }
    }

    pub fn find_by_value(&mut self, search_value: i32) -> Result<Option<Rating>, ()> {
        use crate::schema::ratings::dsl::{rating, ratings};
        let mut connection = self.client.lock().unwrap();

        ratings
            .filter(rating.eq(search_value))
            .select(Rating::as_select())
            .get_result::<Rating>(&mut *connection)
            .optional()
            .or(Err(()))
    }
}
