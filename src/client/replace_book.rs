use std::path::Path;

use chrono::Utc;

use crate::client::utils::combined_author_sort;
use crate::client::*;
use crate::dtos::language::NewLanguageDto;
use crate::dtos::library::ReplaceLibraryEntryDto;
use crate::entities::book::UpdateBookData;
use crate::util::canonicalize_lang;
use crate::Author;

impl CalibreClient {
    pub fn replace_book_metadata(
        &mut self,
        book_id: i32,
        dto: ReplaceLibraryEntryDto,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let author_list = self.replace_book_authors(book_id, dto.authors)?;
        let timestamp = Utc::now();
        let _ = self.client_v2.books().update(
            book_id,
            UpdateBookData {
                author_sort: Some(combined_author_sort(&author_list)),
                title: Some(dto.book.title),
                timestamp: Some(timestamp),
                pubdate: Some(default_pubdate()),
                series_index: None,
                path: None,
                flags: None,
                has_cover: None,
                last_modified: None,
            },
        );

        let book_dir_name = gen_book_folder_name(book_id);
        let book_dir_relative_path = Path::new(&book_dir_name).to_path_buf();

        let publishers = self.replace_book_publishers(book_id, dto.publishers)?;

        let identifiers = dto
            .identifiers
            .into_iter()
            .map(|e| Identifier {
                id: e.id.unwrap(),
                book: book_id,
                type_: e.label,
                val: e.value,
            })
            .collect::<Vec<_>>();

        let language = self.replace_book_language(book_id, dto.language)?;

        let tags = self.replace_book_tags(book_id, dto.tags)?;

        let rating = self.replace_book_rating(book_id, dto.rating)?;

        let metadata = Metadata {
            author_list: &author_list,
            publisher: &publishers,
            identifiers: &identifiers,
            language: language.as_ref(),
            tags: &tags,
            rating: rating.as_ref(),
        };

        let book = self
            .client_v2
            .books()
            .update(
                book_id,
                UpdateBookData {
                    last_modified: Some(Utc::now()),
                    ..Default::default()
                },
            )
            .unwrap();

        let metadata_opf = MetadataOpf::new(&book, &metadata).format();
        match metadata_opf {
            Ok(contents) => {
                let metadata_opf_path = Path::new(&book_dir_relative_path).join("metadata.opf");
                let _ = library_relative_write_file(
                    &self.validated_library_path,
                    &metadata_opf_path,
                    contents.as_bytes(),
                );
            }
            Err(_) => (),
        };

        Ok(())
    }

    fn replace_book_authors(
        &mut self,
        book_id: i32,
        authors: Vec<crate::dtos::author::NewAuthorDto>,
    ) -> Result<Vec<Author>, Box<dyn std::error::Error>> {
        let author_ids = self
            .client_v2
            .books()
            .find_author_ids_by_book_id(book_id)
            .unwrap_or_default();

        for author_id in author_ids {
            let _ = self
                .client_v2
                .books()
                .unlink_author_from_book(book_id, author_id);
        }

        let mut author_list = Vec::new();
        for mut author in authors {
            if author.sortable_name.is_empty() {
                author.sortable_name = Author::sort_author_name_apa(&author.full_name);
            }

            let author = self.client_v2.authors().create_if_missing(author).unwrap();
            let _ = self
                .client_v2
                .books()
                .link_author_to_book(book_id, author.id);
            author_list.push(author);
        }

        Ok(author_list)
    }

    fn replace_book_publishers(
        &mut self,
        book_id: i32,
        publishers: Vec<crate::dtos::publisher::NewPublisherDto>,
    ) -> Result<Vec<Publisher>, Box<dyn std::error::Error>> {
        let publisher_ids = self
            .client_v2
            .books()
            .find_publisher_ids_by_book_id(book_id)
            .unwrap_or_default();

        for publisher_id in publisher_ids {
            let _ = self
                .client_v2
                .books()
                .unlink_publisher_from_book(book_id, publisher_id);
        }

        let mut publisher_list = Vec::new();
        for publisher in publishers {
            let publisher = self
                .client_v2
                .publishers()
                .create_if_missing(publisher)
                .unwrap();
            let _ = self
                .client_v2
                .books()
                .link_publisher_to_book(book_id, publisher.id);
            publisher_list.push(publisher);
        }

        Ok(publisher_list)
    }

    fn replace_book_language(
        &mut self,
        book_id: i32,
        language: Option<crate::dtos::language::NewLanguageDto>,
    ) -> Result<Option<Language>, Box<dyn std::error::Error>> {
        let language_ids = self
            .client_v2
            .books()
            .find_language_ids_by_book_id(book_id)
            .unwrap_or_default();

        for language_id in language_ids {
            let _ = self
                .client_v2
                .books()
                .unlink_language_from_book(book_id, language_id);
        }

        let language = if let Some(language_dto) = language {
            if let Some(canonical_lang) = canonicalize_lang(&language_dto.lang_code) {
                let language = self
                    .client_v2
                    .languages()
                    .create_if_missing(NewLanguageDto {
                        lang_code: canonical_lang.to_639_3().to_string(),
                    })
                    .unwrap();
                let _ = self
                    .client_v2
                    .books()
                    .link_language_to_book(book_id, language.id);
                Some(language)
            } else {
                None
            }
        } else {
            None
        };

        Ok(language)
    }

    fn replace_book_tags(
        &mut self,
        book_id: i32,
        tags: Vec<crate::dtos::tag::NewTagDto>,
    ) -> Result<Vec<Tag>, Box<dyn std::error::Error>> {
        let tag_ids = self
            .client_v2
            .books()
            .find_tag_ids_by_book_id(book_id)
            .unwrap_or_default();

        for tag_id in tag_ids {
            let _ = self.client_v2.books().unlink_tag_from_book(book_id, tag_id);
        }

        let mut tag_list = Vec::new();
        for tag in tags {
            let tag = self.client_v2.tags().create_if_missing(tag).unwrap();
            let _ = self.client_v2.books().link_tag_to_book(book_id, tag.id);
            tag_list.push(tag);
        }

        Ok(tag_list)
    }

    fn replace_book_rating(
        &mut self,
        book_id: i32,
        rating: Option<crate::dtos::rating::NewRatingDto>,
    ) -> Result<Option<Rating>, Box<dyn std::error::Error>> {
        let rating_ids = self
            .client_v2
            .books()
            .find_rating_ids_by_book_id(book_id)
            .unwrap_or_default();

        for rating_id in rating_ids {
            let _ = self
                .client_v2
                .books()
                .unlink_rating_from_book(book_id, rating_id);
        }

        let rating = if let Some(rating_dto) = rating {
            let rating = self
                .client_v2
                .ratings()
                .create_if_missing(rating_dto)
                .unwrap();
            let _ = self
                .client_v2
                .books()
                .link_rating_to_book(book_id, rating.id);
            Some(rating)
        } else {
            None
        };

        Ok(rating)
    }
}
