use crate::cover_image::cover_image_data_from_path;
use crate::dtos::file::NewFileDto;
use crate::dtos::language::NewLanguageDto;
use crate::dtos::library::NewLibraryEntryDto;
use crate::dtos::library::NewLibraryFileDto;
use crate::dtos::publisher::NewPublisherDto;
use crate::dtos::rating::NewRatingDto;
use crate::dtos::tag::NewTagDto;
use crate::entities::book_file::NewBookFile;
use crate::entities::language::Language;
use crate::entities::rating::Rating;
use crate::entities::tag::Tag;
use crate::util::canonicalize_lang;
use crate::Publisher;
use crate::UpsertBookIdentifier;
use chrono::Utc;

use crate::BookFile;

use std::error::Error;
use std::path::Path;
use std::path::PathBuf;

use crate::dtos::author::NewAuthorDto;

use crate::client::*;
use crate::entities::book::{NewBook, UpdateBookData};
use crate::Author;

impl CalibreClient {
    pub fn add_book(&mut self, dto: NewLibraryEntryDto) -> Result<(), Box<dyn std::error::Error>> {
        // 1. Create Authors & Book, then link them.
        // ======================================
        let authors = dto
            .authors
            .into_iter()
            .map(|mut author| {
                if author.sortable_name.is_empty() {
                    author.sortable_name = Author::sort_author_name_apa(&author.full_name);
                }
                author
            })
            .collect::<Vec<NewAuthorDto>>();
        let author_list = self.create_authors(authors)?;
        let creatable_book = NewBook::try_from(dto.book.clone()).unwrap();
        let book_id = self.client_v2.books().create(creatable_book).unwrap().id;
        let timestamp = Utc::now();
        let _ = self.client_v2.books().update(
            book_id,
            UpdateBookData {
                author_sort: Some(combined_author_sort(&author_list)),
                title: None,
                timestamp: Some(timestamp),
                pubdate: Some(default_pubdate()),
                series_index: None,
                path: None,
                flags: None,
                has_cover: None,
                last_modified: None,
            },
        );
        for author in &author_list {
            let _ = self
                .client_v2
                .books()
                .link_author_to_book(book_id, author.id);
        }

        // 2. Create directory for book (removed author directory nesting)
        // ======================================
        let primary_author = &author_list[0].clone();
        let book_dir_name = gen_book_folder_name(book_id);
        let book_dir_relative_path = Path::new(&book_dir_name).to_path_buf();
        library_relative_mkdir(&self.validated_library_path, book_dir_relative_path.clone())?;
        // Update Book with relative path to book folder
        let _ = self
            .client_v2
            .books()
            .update(book_id, update_book_data_for_path(&book_dir_relative_path));

        // 3. Create metadata, then link them.
        // ======================================
        let publishers = self.create_publishers(dto.publishers)?;
        for publisher in publishers.iter() {
            let _ = self
                .client_v2
                .books()
                .link_publisher_to_book(book_id, publisher.id);
        }

        let identifiers = dto
            .identifiers
            .into_iter()
            .map(|i| UpsertBookIdentifier {
                book_id,
                id: i.id,
                label: i.label,
                value: i.value,
            })
            .collect::<Vec<UpsertBookIdentifier>>();
        let identifiers = self.upsert_book_identifiers(identifiers)?;

        let language = if let Some(language_input) = dto.language {
            if let Some(canonical_lang) = canonicalize_lang(&language_input.lang_code) {
                let language = self.create_language(NewLanguageDto {
                    lang_code: canonical_lang.to_639_3().to_string(),
                })?;

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

        let tags = self.create_tags(dto.tags)?;
        for tag in tags.iter() {
            let _ = self.client_v2.books().link_tag_to_book(book_id, tag.id);
        }

        let rating = if let Some(rating) = dto.rating {
            let rating = self.create_rating(rating)?;
            let _ = self
                .client_v2
                .books()
                .link_rating_to_book(book_id, rating.id);
            Some(rating)
        } else {
            None
        };

        let metadata = Metadata {
            author_list: &author_list,
            publisher: &publishers,
            identifiers: &identifiers,
            language: language.as_ref(),
            tags: &tags,
            rating: rating.as_ref(),
        };

        // 4. Copy Book files & cover image to library
        // ===========================
        let mut created_files: Vec<BookFile> = Vec::new();
        if let Some(files) = dto.files {
            // Copy files to library
            let result = self.add_book_files(
                &files,
                &dto.book.title,
                book_id,
                &primary_author.name,
                book_dir_relative_path.clone(),
            );
            if let Ok(files) = result {
                created_files = files;
            }

            let primary_file = &files[0];
            {
                let cover_data = cover_image_data_from_path(primary_file.path.as_path())?;
                if let Some(cover_data) = cover_data {
                    let cover_path = Path::new(&book_dir_relative_path).join("cover.jpg");
                    if library_relative_write_file(
                        &self.validated_library_path,
                        &cover_path,
                        &cover_data,
                    )
                    .is_ok()
                    {
                        let update = UpdateBookData {
                            has_cover: Some(true),
                            ..Default::default()
                        };
                        let _ = self.client_v2.books().update(book_id, update);
                    }
                }
            }
        }

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

        // 5. Create Calibre metadata file
        // ===============================
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

    fn create_authors(
        &mut self,
        authors: Vec<NewAuthorDto>,
    ) -> Result<Vec<Author>, Box<dyn Error>> {
        let x = authors
            .into_iter()
            .map(|dto| self.client_v2.authors().create_if_missing(dto).unwrap())
            .collect::<Vec<Author>>();

        Ok(x)
    }

    fn add_book_files(
        &mut self,
        files: &Vec<NewLibraryFileDto>,
        book_title: &String,
        book_id: i32,
        primary_author_name: &String,
        book_dir_rel_path: PathBuf,
    ) -> Result<Vec<BookFile>, ClientError> {
        let book_files = self.client_v2.book_files();

        files
            .iter()
            .map(|file| {
                let book_file_name = gen_book_file_name(book_title, primary_author_name);
                let nbf = NewBookFile::try_from(NewFileDto {
                    path: file.path.clone(),
                    book_id,
                    name: book_file_name,
                })
                .unwrap();
                let added_book = book_files
                    .create(nbf)
                    .map_err(|_| ClientError::GenericError)?;

                let book_rel_path = Path::new(&book_dir_rel_path).join(&added_book.as_filename());
                let _ = library_relative_copy_file(
                    &self.validated_library_path,
                    file.path.as_path(),
                    book_rel_path.as_path(),
                )
                .map_err(|_| ClientError::GenericError);

                Ok(added_book)
            })
            .collect::<Result<Vec<BookFile>, ClientError>>()
    }

    // === Publishers ===

    fn create_publishers(
        &mut self,
        dto: Vec<NewPublisherDto>,
    ) -> Result<Vec<Publisher>, Box<dyn Error>> {
        let x = dto
            .into_iter()
            .map(|dto| self.client_v2.publishers().create_if_missing(dto).unwrap())
            .collect::<Vec<Publisher>>();

        Ok(x)
    }

    // === Languages ===

    fn create_language(&mut self, dto: NewLanguageDto) -> Result<Language, Box<dyn Error>> {
        Ok(self.client_v2.languages().create_if_missing(dto).unwrap())
    }

    // === Tags ===

    fn create_tags(&mut self, tags: Vec<NewTagDto>) -> Result<Vec<Tag>, Box<dyn Error>> {
        let x = tags
            .into_iter()
            .map(|dto| self.client_v2.tags().create_if_missing(dto).unwrap())
            .collect::<Vec<Tag>>();

        Ok(x)
    }

    // === Ratings ===

    fn create_rating(&mut self, rating: NewRatingDto) -> Result<Rating, Box<dyn Error>> {
        Ok(self.client_v2.ratings().create_if_missing(rating).unwrap())
    }
}
