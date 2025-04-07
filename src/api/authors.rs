use std::sync::Arc;
use std::sync::Mutex;

use diesel::prelude::*;
use diesel::SelectableHelper;

use crate::dtos::author::NewAuthorDto;
use crate::dtos::author::UpdateAuthorDto;
use crate::entities::author::NewAuthor;
use crate::entities::author::UpdateAuthorData;
use crate::Author;

pub struct AuthorsHandler {
    client: Arc<Mutex<SqliteConnection>>,
}

impl AuthorsHandler {
    pub(crate) fn new(client: Arc<Mutex<SqliteConnection>>) -> Self {
        Self { client }
    }

    pub fn list(&self) -> Result<Vec<Author>, ()> {
        use crate::schema::authors::dsl::*;
        let mut connection = self.client.lock().unwrap();

        authors
            .select(Author::as_select())
            .load::<Author>(&mut *connection)
            .or(Err(()))
    }

    pub fn create(&mut self, dto: NewAuthorDto) -> Result<Author, ()> {
        use crate::schema::authors::dsl::*;
        let new_author = NewAuthor::try_from(dto)?;
        let mut connection = self.client.lock().unwrap();

        diesel::insert_into(authors)
            .values(new_author)
            .returning(Author::as_returning())
            .get_result::<Author>(&mut *connection)
            .or(Err(()))
    }

    pub fn create_if_missing(&mut self, dto: NewAuthorDto) -> Result<Author, ()> {
        match self.find_by_name(&dto.full_name)? {
            Some(author) => Ok(author),
            _ => self.create(dto),
        }
    }

    pub fn find_by_id(&mut self, search_id: i32) -> Result<Option<Author>, ()> {
        use crate::schema::authors::dsl::*;
        let mut connection = self.client.lock().unwrap();

        authors
            .filter(id.eq(search_id))
            .select(Author::as_select())
            .get_result::<Author>(&mut *connection)
            .optional()
            .or(Err(()))
    }

    pub fn find_by_name(&mut self, search_name: &str) -> Result<Option<Author>, ()> {
        use crate::schema::authors::dsl::*;
        let mut connection = self.client.lock().unwrap();

        authors
            .filter(name.eq(search_name))
            .select(Author::as_select())
            .get_result::<Author>(&mut *connection)
            .optional()
            .or(Err(()))
    }

    pub fn update(&mut self, author_id: i32, dto: UpdateAuthorDto) -> Result<Author, ()> {
        use crate::schema::authors::dsl::*;
        let mut connection = self.client.lock().unwrap();
        let author = UpdateAuthorData::try_from(dto)?;

        diesel::update(authors)
            .filter(id.eq(author_id))
            .set(author)
            .returning(Author::as_returning())
            .get_result(&mut *connection)
            .or(Err(()))
    }

    pub fn name_author_dir(&mut self, author: &Author) -> String {
        author.name.clone()
    }

    pub fn replace_with_translation(
        &mut self,
        author_id: i32,
        translation: &str,
    ) -> Result<(), ()> {
        let translated_author = self.find_by_name(translation)?;

        if translated_author.is_none() {
            self.update_author_name(author_id, translation)?;
        } else {
            let translated_author = translated_author.unwrap();
            self.transfer_author_links_and_delete(author_id, translated_author.id)?;
        }
        Ok(())
    }

    fn update_author_name(&mut self, author_id: i32, new_name: &str) -> Result<(), ()> {
        use crate::schema::authors::dsl::{authors, id, name};
        let mut connection = self.client.lock().unwrap();

        diesel::update(authors.filter(id.eq(author_id)))
            .set(name.eq(new_name))
            .execute(&mut *connection)
            .or(Err(()))?;

        Ok(())
    }

    fn transfer_author_links_and_delete(
        &mut self,
        from_author_id: i32,
        to_author_id: i32,
    ) -> Result<(), ()> {
        use crate::schema::authors::dsl::authors;
        use crate::schema::books_authors_link::dsl::{author, book, books_authors_link, id};

        let mut connection = self.client.lock().unwrap();

        connection
            .transaction::<_, diesel::result::Error, _>(|conn| {
                let book_ids = books_authors_link
                    .filter(author.eq(from_author_id))
                    .select(book)
                    .load::<i32>(conn)
                    .map_err(|_| diesel::result::Error::RollbackTransaction)?;

                for book_id in book_ids {
                    let already_linked = books_authors_link
                        .filter(book.eq(book_id).and(author.eq(to_author_id)))
                        .select(id)
                        .first::<i32>(conn)
                        .optional()
                        .map_err(|_| diesel::result::Error::RollbackTransaction)?
                        .is_some();

                    if already_linked {
                        diesel::delete(
                            books_authors_link
                                .filter(author.eq(from_author_id).and(book.eq(book_id))),
                        )
                        .execute(conn)
                        .map_err(|_| diesel::result::Error::RollbackTransaction)?;
                    } else {
                        diesel::update(
                            books_authors_link
                                .filter(author.eq(from_author_id).and(book.eq(book_id))),
                        )
                        .set(author.eq(to_author_id))
                        .execute(conn)
                        .map_err(|_| diesel::result::Error::RollbackTransaction)?;
                    }
                }

                diesel::delete(authors.find(from_author_id))
                    .execute(conn)
                    .map_err(|_| diesel::result::Error::RollbackTransaction)?;

                Ok(())
            })
            .or(Err(()))
    }

    pub fn get_all_authors(&mut self) -> Result<Vec<Author>, ()> {
        use crate::schema::authors::dsl::{authors, id};
        let mut connection = self.client.lock().unwrap();

        authors
            .select(Author::as_select())
            .order(id.asc())
            .get_results::<Author>(&mut *connection)
            .or(Err(()))
    }
}
