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
}
