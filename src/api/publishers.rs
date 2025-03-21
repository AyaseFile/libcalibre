use std::sync::Arc;
use std::sync::Mutex;

use diesel::prelude::*;

use crate::dtos::publisher::NewPublisherDto;
use crate::entities::publisher::NewPublisher;
use crate::Publisher;

pub struct PublishersHandler {
    client: Arc<Mutex<SqliteConnection>>,
}

impl PublishersHandler {
    pub(crate) fn new(client: Arc<Mutex<SqliteConnection>>) -> Self {
        Self { client }
    }

    pub fn create(&mut self, dto: NewPublisherDto) -> Result<Publisher, ()> {
        use crate::schema::publishers::dsl::publishers;
        let new_publisher = NewPublisher::try_from(dto)?;
        let mut connection = self.client.lock().unwrap();

        diesel::insert_into(publishers)
            .values(new_publisher)
            .returning(Publisher::as_returning())
            .get_result::<Publisher>(&mut *connection)
            .or(Err(()))
    }

    pub fn create_if_missing(&mut self, dto: NewPublisherDto) -> Result<Publisher, ()> {
        match self.find_by_name(&dto.name)? {
            Some(publisher) => Ok(publisher),
            _ => self.create(dto),
        }
    }

    pub fn find_by_name(&mut self, search_name: &str) -> Result<Option<Publisher>, ()> {
        use crate::schema::publishers::dsl::{name, publishers};
        let mut connection = self.client.lock().unwrap();

        publishers
            .filter(name.eq(search_name))
            .select(Publisher::as_select())
            .get_result::<Publisher>(&mut *connection)
            .optional()
            .or(Err(()))
    }
}
