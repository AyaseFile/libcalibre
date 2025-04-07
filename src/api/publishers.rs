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

    pub fn replace_with_translation(
        &mut self,
        publisher_id: i32,
        translation: &str,
    ) -> Result<(), ()> {
        let translated_publisher = self.find_by_name(translation)?;

        if translated_publisher.is_none() {
            self.update_publisher_name(publisher_id, translation)?;
        } else {
            let translated_publisher = translated_publisher.unwrap();
            self.transfer_publisher_links_and_delete(publisher_id, translated_publisher.id)?;
        }
        Ok(())
    }

    fn update_publisher_name(&mut self, publisher_id: i32, new_name: &str) -> Result<(), ()> {
        use crate::schema::publishers::dsl::{id, name, publishers};
        let mut connection = self.client.lock().unwrap();

        diesel::update(publishers.filter(id.eq(publisher_id)))
            .set(name.eq(new_name))
            .execute(&mut *connection)
            .or(Err(()))?;

        Ok(())
    }

    fn transfer_publisher_links_and_delete(
        &mut self,
        from_publisher_id: i32,
        to_publisher_id: i32,
    ) -> Result<(), ()> {
        use crate::schema::books_publishers_link::dsl::{
            book, books_publishers_link, id, publisher,
        };
        use crate::schema::publishers::dsl::publishers;

        let mut connection = self.client.lock().unwrap();

        connection
            .transaction::<_, diesel::result::Error, _>(|conn| {
                let book_ids = books_publishers_link
                    .filter(publisher.eq(from_publisher_id))
                    .select(book)
                    .load::<i32>(conn)
                    .map_err(|_| diesel::result::Error::RollbackTransaction)?;

                for book_id in book_ids {
                    let already_linked = books_publishers_link
                        .filter(book.eq(book_id).and(publisher.eq(to_publisher_id)))
                        .select(id)
                        .first::<i32>(conn)
                        .optional()
                        .map_err(|_| diesel::result::Error::RollbackTransaction)?
                        .is_some();

                    if already_linked {
                        diesel::delete(
                            books_publishers_link
                                .filter(publisher.eq(from_publisher_id).and(book.eq(book_id))),
                        )
                        .execute(conn)
                        .map_err(|_| diesel::result::Error::RollbackTransaction)?;
                    } else {
                        diesel::update(
                            books_publishers_link
                                .filter(publisher.eq(from_publisher_id).and(book.eq(book_id))),
                        )
                        .set(publisher.eq(to_publisher_id))
                        .execute(conn)
                        .map_err(|_| diesel::result::Error::RollbackTransaction)?;
                    }
                }

                diesel::delete(publishers.find(from_publisher_id))
                    .execute(conn)
                    .map_err(|_| diesel::result::Error::RollbackTransaction)?;

                Ok(())
            })
            .or(Err(()))
    }

    pub fn get_all_publishers(&mut self) -> Result<Vec<Publisher>, ()> {
        use crate::schema::publishers::dsl::{id, publishers};
        let mut connection = self.client.lock().unwrap();

        publishers
            .select(Publisher::as_select())
            .order(id.asc())
            .get_results::<Publisher>(&mut *connection)
            .or(Err(()))
    }
}
