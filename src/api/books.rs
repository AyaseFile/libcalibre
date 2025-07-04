use std::sync::Arc;
use std::sync::Mutex;

use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::Integer;
use diesel::QueryableByName;

use crate::entities::book::{NewBook, UpdateBookData, UpsertBookIdentifier};
use crate::models::Identifier;
use crate::Book;

#[derive(QueryableByName)]
struct CustomValue {
    #[diesel(sql_type = Integer)]
    value: i32,
}

pub struct BooksHandler {
    client: Arc<Mutex<SqliteConnection>>,
}

impl BooksHandler {
    pub(crate) fn new(client: Arc<Mutex<SqliteConnection>>) -> Self {
        Self { client }
    }

    pub fn create(&self, new_book: NewBook) -> Result<Book, ()> {
        use crate::schema::books::dsl::*;
        let mut connection = self.client.lock().unwrap();

        let b = diesel::insert_into(books)
            .values(new_book)
            .returning(Book::as_returning())
            .get_result(&mut *connection)
            .expect("Error saving new book");

        // SQLite doesn't add the UUID until after our `insert_into` call,
        // so we need to fetch it from the DB to provide it to the caller.
        let mut book_generated = b.clone();
        let book_uuid = uuid_for_book(&mut *connection, b.id);
        book_generated.uuid = book_uuid;

        Ok(book_generated)
    }

    pub fn list(&self) -> Result<Vec<Book>, ()> {
        use crate::schema::books::dsl::*;
        let mut connection = self.client.lock().unwrap();

        books
            .select(Book::as_select())
            .load::<Book>(&mut *connection)
            .or(Err(()))
    }

    pub fn update(&mut self, book_id: i32, book: UpdateBookData) -> Result<Book, ()> {
        use crate::schema::books::dsl::*;
        let mut connection = self.client.lock().unwrap();

        diesel::update(books)
            .filter(id.eq(book_id))
            .set(book)
            .returning(Book::as_returning())
            .get_result(&mut *connection)
            .or(Err(()))
    }

    pub fn find_by_id(&mut self, search_id: i32) -> Result<Option<Book>, ()> {
        use crate::schema::books::dsl::*;
        let mut connection = self.client.lock().unwrap();

        books
            .filter(id.eq(search_id))
            .select(Book::as_select())
            .get_result::<Book>(&mut *connection)
            .optional()
            .or(Err(()))
    }

    pub fn find_author_ids_by_book_id(&mut self, book_id: i32) -> Result<Vec<i32>, ()> {
        use crate::schema::books_authors_link::dsl::*;
        let mut connection = self.client.lock().unwrap();

        let author_ids = books_authors_link
            .filter(book.eq(book_id))
            .select(author)
            .load::<i32>(&mut *connection);

        match author_ids {
            Ok(ids) => Ok(ids),
            Err(_) => Err(()),
        }
    }

    pub fn link_author_to_book(&mut self, book_id: i32, author_id: i32) -> Result<(), ()> {
        use crate::schema::books_authors_link::dsl::*;
        let mut connection = self.client.lock().unwrap();

        diesel::insert_into(books_authors_link)
            .values((book.eq(book_id), author.eq(author_id)))
            .execute(&mut *connection)
            .map(|_| ())
            .or(Err(()))
    }

    pub fn unlink_author_from_book(&mut self, book_id: i32, author_id: i32) -> Result<(), ()> {
        use crate::schema::books_authors_link::dsl::*;
        let mut connection = self.client.lock().unwrap();

        diesel::delete(books_authors_link.filter(book.eq(book_id).and(author.eq(author_id))))
            .execute(&mut *connection)
            .map(|_| ())
            .or(Err(()))
    }

    // === === ===
    // Identifiers
    // === === ===

    pub fn list_identifiers_for_book(&mut self, book_id: i32) -> Result<Vec<Identifier>, ()> {
        use crate::schema::identifiers::dsl::*;
        let mut connection = self.client.lock().unwrap();

        identifiers
            .filter(book.eq(book_id))
            .select(Identifier::as_returning())
            .load(&mut *connection)
            .or(Err(()))
    }

    pub fn upsert_book_identifier(
        &mut self,
        update: UpsertBookIdentifier,
    ) -> Result<Identifier, ()> {
        match update.id {
            Some(update_id) => self.update_book_identifier(update, update_id),
            None => self.create_book_identifier(update),
        }
    }

    fn update_book_identifier(
        &mut self,
        update: UpsertBookIdentifier,
        identifier_id: i32,
    ) -> Result<Identifier, ()> {
        use crate::schema::identifiers::dsl::{id, identifiers, type_, val};
        let mut connection = self.client.lock().unwrap();

        diesel::update(identifiers)
            .filter(id.eq(identifier_id))
            .set((type_.eq(update.label), val.eq(update.value)))
            .returning(Identifier::as_returning())
            .get_result::<Identifier>(&mut *connection)
            .or(Err(()))
    }

    fn create_book_identifier(&mut self, update: UpsertBookIdentifier) -> Result<Identifier, ()> {
        use crate::schema::identifiers::dsl::{book, identifiers, type_, val};
        let mut connection = self.client.lock().unwrap();
        let lowercased_label = update.label.to_lowercase();

        diesel::insert_into(identifiers)
            .values((
                book.eq(update.book_id),
                type_.eq(lowercased_label),
                val.eq(update.value),
            ))
            .returning(Identifier::as_returning())
            .get_result::<Identifier>(&mut *connection)
            .or(Err(()))
    }

    pub fn delete_book_identifier(&mut self, book_id: i32, identifier_id: i32) -> Result<(), ()> {
        use crate::schema::identifiers::dsl::{book, id, identifiers};
        let mut connection = self.client.lock().unwrap();

        diesel::delete(identifiers.filter(book.eq(book_id).and(id.eq(identifier_id))))
            .execute(&mut *connection)
            .map(|_| ())
            .or(Err(()))
    }

    // === === ===
    // Descriptions
    // === === ===

    pub fn get_description(&mut self, book_id: i32) -> Result<Option<String>, ()> {
        use crate::schema::comments::dsl::*;
        let mut connection = self.client.lock().unwrap();

        comments
            .filter(book.eq(book_id))
            .select(text)
            .first(&mut *connection)
            .optional()
            .or(Err(()))
    }

    // === === ===
    // Read state
    // === === ===
    fn get_or_create_read_state_custom_column(
        &self,
        connection: &mut SqliteConnection,
    ) -> Result<i32, ()> {
        use crate::schema::custom_columns::dsl::*;

        let custom_column_id = custom_columns
            .select(id)
            .filter(label.eq("read"))
            .filter(datatype.eq("bool"))
            .first::<i32>(connection)
            .optional()
            .or(Err(()))?;

        if custom_column_id.is_none() {
            let column_id = diesel::insert_into(custom_columns)
                .values((
                    label.eq("read"),
                    name.eq("Read"),
                    datatype.eq("bool"),
                    mark_for_delete.eq(false),
                    editable.eq(true),
                    is_multiple.eq(false),
                    normalized.eq(false),
                    display.eq("{}"),
                ))
                .returning(id)
                .get_result::<i32>(connection)
                .or(Err(()))?;

            sql_query(format!(
                "CREATE TABLE custom_column_{column_id} (id INTEGER PRIMARY KEY, book INTEGER NOT NULL, value INTEGER NOT NULL);"
            ))
            .execute(connection)
            .or(Err(()))?;

            Ok(column_id)
        } else {
            Ok(custom_column_id.unwrap())
        }
    }

    pub fn get_book_read_state(&self, book_id: i32) -> Result<Option<bool>, ()> {
        let mut connection = self.client.lock().unwrap();

        let read_state_column_id = self.get_or_create_read_state_custom_column(&mut *connection)?;
        let value = sql_query(format!(
            "SELECT value FROM custom_column_{read_state_column_id} WHERE book = ?"
        ))
        .bind::<Integer, _>(book_id)
        .get_result::<CustomValue>(&mut *connection)
        .map(|v| v.value)
        .or(Err(()));

        match value {
            Ok(v) => match v {
                1 => Ok(Some(true)),
                _ => Ok(Some(false)),
            },
            Err(_) => Ok(Some(false)),
        }
    }

    pub fn set_book_read_state(&mut self, book_id: i32, read_state: bool) -> Result<(), ()> {
        let mut connection = self.client.lock().unwrap();

        let read_state_column_id = self.get_or_create_read_state_custom_column(&mut *connection)?;
        let value = if read_state { 1 } else { 0 };

        sql_query(format!(
            "INSERT OR REPLACE INTO custom_column_{read_state_column_id} (book, value) VALUES (?, ?)"
        ))
        .bind::<Integer, _>(book_id)
        .bind::<Integer, _>(value)
        .execute(&mut *connection)
        .map(|_| ())
        .or(Err(()))
    }

    // === === ===
    // Publishers
    // === === ===

    pub fn link_publisher_to_book(&mut self, book_id: i32, publisher_id: i32) -> Result<(), ()> {
        use crate::schema::books_publishers_link::dsl::{book, books_publishers_link, publisher};
        let mut connection = self.client.lock().unwrap();

        diesel::insert_into(books_publishers_link)
            .values((book.eq(book_id), publisher.eq(publisher_id)))
            .execute(&mut *connection)
            .map(|_| ())
            .or(Err(()))
    }

    pub fn unlink_publisher_from_book(
        &mut self,
        book_id: i32,
        publisher_id: i32,
    ) -> Result<(), ()> {
        use crate::schema::books_publishers_link::dsl::{book, books_publishers_link, publisher};
        let mut connection = self.client.lock().unwrap();

        diesel::delete(
            books_publishers_link.filter(book.eq(book_id).and(publisher.eq(publisher_id))),
        )
        .execute(&mut *connection)
        .map(|_| ())
        .or(Err(()))
    }

    // === === ===
    // Tags
    // === === ===

    pub fn link_tag_to_book(&mut self, book_id: i32, tag_id: i32) -> Result<(), ()> {
        use crate::schema::books_tags_link::dsl::{book, books_tags_link, tag};
        let mut connection = self.client.lock().unwrap();

        diesel::insert_into(books_tags_link)
            .values((book.eq(book_id), tag.eq(tag_id)))
            .execute(&mut *connection)
            .map(|_| ())
            .or(Err(()))
    }

    pub fn unlink_tag_from_book(&mut self, book_id: i32, tag_id: i32) -> Result<(), ()> {
        use crate::schema::books_tags_link::dsl::{book, books_tags_link, tag};
        let mut connection = self.client.lock().unwrap();

        diesel::delete(books_tags_link.filter(book.eq(book_id).and(tag.eq(tag_id))))
            .execute(&mut *connection)
            .map(|_| ())
            .or(Err(()))
    }

    // === === ===
    // Ratings
    // === === ===

    pub fn link_rating_to_book(&mut self, book_id: i32, rating_id: i32) -> Result<(), ()> {
        use crate::schema::books_ratings_link::dsl::{book, books_ratings_link, rating};
        let mut connection = self.client.lock().unwrap();

        let _ =
            diesel::delete(books_ratings_link.filter(book.eq(book_id))).execute(&mut *connection);
        diesel::insert_into(books_ratings_link)
            .values((book.eq(book_id), rating.eq(rating_id)))
            .execute(&mut *connection)
            .map(|_| ())
            .or(Err(()))
    }

    pub fn unlink_rating_from_book(&mut self, book_id: i32, rating_id: i32) -> Result<(), ()> {
        use crate::schema::books_ratings_link::dsl::{book, books_ratings_link, rating};
        let mut connection = self.client.lock().unwrap();

        diesel::delete(books_ratings_link.filter(book.eq(book_id).and(rating.eq(rating_id))))
            .execute(&mut *connection)
            .map(|_| ())
            .or(Err(()))
    }

    // === === ===
    // Languages
    // === === ===

    pub fn link_language_to_book(&mut self, book_id: i32, language_id: i32) -> Result<(), ()> {
        use crate::schema::books_languages_link::dsl::{book, books_languages_link, lang_code};
        let mut connection = self.client.lock().unwrap();

        diesel::insert_into(books_languages_link)
            .values((book.eq(book_id), lang_code.eq(language_id)))
            .execute(&mut *connection)
            .map(|_| ())
            .or(Err(()))
    }

    pub fn unlink_language_from_book(&mut self, book_id: i32, language_id: i32) -> Result<(), ()> {
        use crate::schema::books_languages_link::dsl::{book, books_languages_link, lang_code};
        let mut connection = self.client.lock().unwrap();

        diesel::delete(books_languages_link.filter(book.eq(book_id).and(lang_code.eq(language_id))))
            .execute(&mut *connection)
            .map(|_| ())
            .or(Err(()))
    }

    pub fn find_publisher_ids_by_book_id(&mut self, book_id: i32) -> Result<Vec<i32>, ()> {
        use crate::schema::books_publishers_link::dsl::*;
        let mut connection = self.client.lock().unwrap();

        books_publishers_link
            .filter(book.eq(book_id))
            .select(publisher)
            .load::<i32>(&mut *connection)
            .or(Err(()))
    }

    pub fn find_language_ids_by_book_id(&mut self, book_id: i32) -> Result<Vec<i32>, ()> {
        use crate::schema::books_languages_link::dsl::*;
        let mut connection = self.client.lock().unwrap();

        books_languages_link
            .filter(book.eq(book_id))
            .select(lang_code)
            .load::<i32>(&mut *connection)
            .or(Err(()))
    }

    pub fn find_identifier_ids_by_book_id(&mut self, book_id: i32) -> Result<Vec<i32>, ()> {
        use crate::schema::identifiers::dsl::*;
        let mut connection = self.client.lock().unwrap();

        identifiers
            .filter(book.eq(book_id))
            .select(id)
            .load::<i32>(&mut *connection)
            .or(Err(()))
    }

    pub fn find_tag_ids_by_book_id(&mut self, book_id: i32) -> Result<Vec<i32>, ()> {
        use crate::schema::books_tags_link::dsl::*;
        let mut connection = self.client.lock().unwrap();

        books_tags_link
            .filter(book.eq(book_id))
            .select(tag)
            .load::<i32>(&mut *connection)
            .or(Err(()))
    }

    pub fn find_book_id_by_identifier(
        &self,
        i_type: &str,
        i_value: &str,
    ) -> Result<Option<i32>, String> {
        use crate::schema::identifiers::dsl::{book, identifiers, type_, val};
        let mut connection = self.client.lock().unwrap();

        let results: Vec<i32> = identifiers
            .filter(type_.eq(i_type))
            .filter(val.eq(i_value))
            .select(book)
            .load(&mut *connection)
            .map_err(|e| format!("Database query failed: {e}"))?;

        match results.len() {
            0 => Ok(None),
            1 => Ok(Some(results[0])),
            _ => Err(format!(
                "Multiple books found for identifier {i_type}:{i_value}"
            )),
        }
    }

    pub fn find_rating_ids_by_book_id(&mut self, book_id: i32) -> Result<Vec<i32>, ()> {
        use crate::schema::books_ratings_link::dsl::*;
        let mut connection = self.client.lock().unwrap();

        books_ratings_link
            .filter(book.eq(book_id))
            .select(rating)
            .load::<i32>(&mut *connection)
            .or(Err(()))
    }
}

fn uuid_for_book(conn: &mut SqliteConnection, book_id: i32) -> Option<String> {
    use crate::schema::books::dsl::*;

    books
        .select(uuid)
        .filter(id.eq(book_id))
        .first::<Option<String>>(conn)
        .expect("Error getting book UUID")
}
