use std::sync::Arc;
use std::sync::Mutex;

use diesel::prelude::*;

use crate::dtos::tag::NewTagDto;
use crate::entities::tag::NewTag;
use crate::Tag;

pub struct TagsHandler {
    client: Arc<Mutex<SqliteConnection>>,
}

impl TagsHandler {
    pub(crate) fn new(client: Arc<Mutex<SqliteConnection>>) -> Self {
        Self { client }
    }

    pub fn create(&mut self, dto: NewTagDto) -> Result<Tag, ()> {
        use crate::schema::tags::dsl::tags;
        let new_tag = NewTag::try_from(dto)?;
        let mut connection = self.client.lock().unwrap();

        diesel::insert_into(tags)
            .values(new_tag)
            .returning(Tag::as_returning())
            .get_result::<Tag>(&mut *connection)
            .or(Err(()))
    }

    pub fn create_if_missing(&mut self, dto: NewTagDto) -> Result<Tag, ()> {
        match self.find_by_name(&dto.name)? {
            Some(tag) => Ok(tag),
            _ => self.create(dto),
        }
    }

    pub fn find_by_name(&mut self, search_name: &str) -> Result<Option<Tag>, ()> {
        use crate::schema::tags::dsl::{name, tags};
        let mut connection = self.client.lock().unwrap();

        tags.filter(name.eq(search_name))
            .select(Tag::as_select())
            .get_result::<Tag>(&mut *connection)
            .optional()
            .or(Err(()))
    }

    pub fn replace_with_translation(&mut self, tag_id: i32, translation: &str) -> Result<(), ()> {
        let translated_tag = self.find_by_name(translation)?;

        if translated_tag.is_none() {
            self.update_tag_name(tag_id, translation)?;
        } else {
            let translated_tag = translated_tag.unwrap();
            self.transfer_tag_links_and_delete(tag_id, translated_tag.id)?;
        }
        Ok(())
    }

    fn update_tag_name(&mut self, tag_id: i32, new_name: &str) -> Result<(), ()> {
        use crate::schema::tags::dsl::{id, name, tags};
        let mut connection = self.client.lock().unwrap();

        diesel::update(tags.filter(id.eq(tag_id)))
            .set(name.eq(new_name))
            .execute(&mut *connection)
            .or(Err(()))?;

        Ok(())
    }

    fn transfer_tag_links_and_delete(
        &mut self,
        from_tag_id: i32,
        to_tag_id: i32,
    ) -> Result<(), ()> {
        use crate::schema::books_tags_link::dsl::{book, books_tags_link, id, tag};
        use crate::schema::tags::dsl::tags;

        let mut connection = self.client.lock().unwrap();

        connection
            .transaction::<_, diesel::result::Error, _>(|conn| {
                let book_ids = books_tags_link
                    .filter(tag.eq(from_tag_id))
                    .select(book)
                    .load::<i32>(conn)
                    .map_err(|_| diesel::result::Error::RollbackTransaction)?;

                for book_id in book_ids {
                    let already_linked = books_tags_link
                        .filter(book.eq(book_id).and(tag.eq(to_tag_id)))
                        .select(id)
                        .first::<i32>(conn)
                        .optional()
                        .map_err(|_| diesel::result::Error::RollbackTransaction)?
                        .is_some();

                    if already_linked {
                        diesel::delete(
                            books_tags_link.filter(tag.eq(from_tag_id).and(book.eq(book_id))),
                        )
                        .execute(conn)
                        .map_err(|_| diesel::result::Error::RollbackTransaction)?;
                    } else {
                        diesel::update(
                            books_tags_link.filter(tag.eq(from_tag_id).and(book.eq(book_id))),
                        )
                        .set(tag.eq(to_tag_id))
                        .execute(conn)
                        .map_err(|_| diesel::result::Error::RollbackTransaction)?;
                    }
                }

                diesel::delete(tags.find(from_tag_id))
                    .execute(conn)
                    .map_err(|_| diesel::result::Error::RollbackTransaction)?;

                Ok(())
            })
            .or(Err(()))
    }

    pub fn get_all_tags(&mut self) -> Result<Vec<Tag>, ()> {
        use crate::schema::tags::dsl::{id, tags};
        let mut connection = self.client.lock().unwrap();

        tags.select(Tag::as_select())
            .order(id.asc())
            .get_results::<Tag>(&mut *connection)
            .or(Err(()))
    }
}
