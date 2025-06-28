use crate::dtos::library::UpdateLibraryEntryDto;

use crate::client::*;
use crate::entities::book::UpdateBookData;

impl CalibreClient {
    pub fn update_book(
        &mut self,
        book_id: i32,
        updates: UpdateLibraryEntryDto,
    ) -> Result<crate::BookWithAuthorsAndFiles, Box<dyn std::error::Error>> {
        // Write new updates to book
        let is_read = updates.book.is_read;
        let book_update = UpdateBookData::try_from(updates.book).unwrap();
        let _book = self.client_v2.books().update(book_id, book_update);

        if is_read.is_some() {
            let _set_book_result = self
                .client_v2
                .books()
                .set_book_read_state(book_id, is_read.unwrap());
        }

        match updates.author_id_list {
            Some(author_id_list) => {
                // Unlink existing authors
                let existing_authors = self
                    .client_v2
                    .books()
                    .find_author_ids_by_book_id(book_id)
                    .unwrap();
                existing_authors.iter().for_each(|&author_id| {
                    let _ = self
                        .client_v2
                        .books()
                        .unlink_author_from_book(book_id, author_id);
                });

                // Link requested authors to book
                author_id_list.iter().for_each(|author_id| {
                    let author_id_int = author_id.parse::<i32>().unwrap();
                    let _ = self
                        .client_v2
                        .books()
                        .link_author_to_book(book_id, author_id_int);
                });
            }
            None => {}
        }

        self.find_book_with_authors(book_id)
    }
}
