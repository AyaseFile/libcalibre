use crate::cover_image::cover_image_data_from_path;
use crate::dtos::author::UpdateAuthorDto;
use crate::dtos::file::NewFileDto;
use crate::dtos::language::NewLanguageDto;
use crate::dtos::library::NewLibraryEntryDto;
use crate::dtos::library::NewLibraryFileDto;
use crate::dtos::library::UpdateLibraryEntryDto;
use crate::dtos::publisher::NewPublisherDto;
use crate::dtos::rating::NewRatingDto;
use crate::dtos::tag::NewTagDto;
use crate::entities::book_file::NewBookFile;
use crate::entities::language::Language;
use crate::entities::rating::Rating;
use crate::entities::tag::Tag;
use crate::util::canonicalize_lang;
use crate::Book;
use crate::Publisher;
use crate::UpsertBookIdentifier;
use chrono::DateTime;
use chrono::Utc;
use deunicode::deunicode;
use sanitise_file_name::sanitise;

use crate::BookFile;

use std::error::Error;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use diesel::RunQueryDsl;

use crate::dtos::author::NewAuthorDto;

use crate::entities::book::{NewBook, UpdateBookData};
use crate::models::Identifier;
use crate::persistence::establish_connection;
use crate::util::ValidDbPath;
use crate::Author;
use crate::BookWithAuthorsAndFiles;
use crate::ClientV2;

#[derive(Debug)]
pub enum CalibreError {
    DatabaseError,
}

impl std::fmt::Display for CalibreError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for CalibreError {}

pub struct CalibreClient {
    validated_library_path: ValidDbPath,
    client_v2: ClientV2,
}

impl CalibreClient {
    pub fn new(db_path: ValidDbPath) -> CalibreClient {
        CalibreClient {
            validated_library_path: db_path.clone(),
            client_v2: ClientV2::new(db_path),
        }
    }

    pub fn add_book(
        &mut self,
        dto: NewLibraryEntryDto,
    ) -> Result<crate::BookWithAuthorsAndFiles, Box<dyn std::error::Error>> {
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

        // 2. Create directories for author & book
        // ======================================
        let primary_author = &author_list[0].clone();
        let author_dir_name = self.client_v2.authors().name_author_dir(primary_author);

        let book_dir_name = gen_book_folder_name(book_id);
        let book_dir_relative_path = Path::new(&author_dir_name).join(&book_dir_name);
        library_relative_mkdir(
            &self.validated_library_path,
            Path::new(&author_dir_name).to_path_buf(),
        )?;
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

        Ok(BookWithAuthorsAndFiles {
            book,
            authors: author_list,
            files: created_files,
            book_description_html: None,
            is_read: false,
        })
    }

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

    pub fn find_book_with_authors(
        &mut self,
        book_id: i32,
    ) -> Result<crate::BookWithAuthorsAndFiles, Box<dyn std::error::Error>> {
        let book = self.client_v2.books().find_by_id(book_id).unwrap().unwrap();
        let book_desc = self.client_v2.books().get_description(book_id).unwrap();
        let author_ids = self
            .client_v2
            .books()
            .find_author_ids_by_book_id(book_id)
            .unwrap();

        let authors: Vec<Author> = author_ids
            .into_iter()
            .map(|author_id| {
                let authors = self.client_v2.authors().find_by_id(author_id);
                match authors {
                    Ok(Some(author)) => Ok(author),
                    _ => Err(ClientError::GenericError),
                }
            })
            .map(|item| item.map_err(|e| e.into()))
            .collect::<Result<Vec<Author>, Box<dyn Error>>>()?;

        let files = self
            .client_v2
            .book_files()
            .list_all_by_book_id(book.id)
            .map_err(|_| ClientError::GenericError)?;

        let is_read = self
            .client_v2
            .books()
            .get_book_read_state(book.id)
            .unwrap_or(Some(false))
            .unwrap_or(false);

        Ok(BookWithAuthorsAndFiles {
            book,
            authors,
            files,
            book_description_html: book_desc,
            is_read,
        })
    }

    pub fn find_all(
        &mut self,
    ) -> Result<Vec<crate::BookWithAuthorsAndFiles>, Box<dyn std::error::Error>> {
        let mut book_list = Vec::new();
        let books = self.client_v2.books().list().unwrap();

        for book in books {
            let result = self.find_book_with_authors(book.id);
            match result {
                Ok(res) => book_list.push(res),
                Err(_) => (),
            }
        }

        Ok(book_list)
    }

    pub fn list_all_authors(&mut self) -> Result<Vec<crate::Author>, Box<dyn std::error::Error>> {
        self.client_v2
            .authors()
            .list()
            .map_err(|_| Box::new(CalibreError::DatabaseError) as Box<dyn std::error::Error>)
    }

    pub fn list_identifiers_for_book(
        &mut self,
        book_id: i32,
    ) -> Result<Vec<Identifier>, Box<dyn std::error::Error>> {
        self.client_v2
            .books()
            .list_identifiers_for_book(book_id)
            .map_err(|_| Box::new(CalibreError::DatabaseError) as Box<dyn std::error::Error>)
    }

    pub fn update_author(
        &mut self,
        author_id: i32,
        updates: UpdateAuthorDto,
    ) -> Result<crate::Author, ()> {
        self.client_v2.authors().update(author_id, updates)
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

    // === Identifiers ===

    pub fn upsert_book_identifiers(
        &mut self,
        update: Vec<UpsertBookIdentifier>,
    ) -> Result<Vec<Identifier>, Box<dyn Error>> {
        let x = update
            .into_iter()
            .map(|dto| self.client_v2.books().upsert_book_identifier(dto).unwrap())
            .collect::<Vec<Identifier>>();

        Ok(x)
    }

    pub fn delete_book_identifier(&mut self, book_id: i32, identifier_id: i32) -> Result<(), ()> {
        self.client_v2
            .books()
            .delete_book_identifier(book_id, identifier_id)
    }

    /// Updates the library's ID to a new UUID.
    ///
    /// You probably do not need this method, unless you're creating a new
    /// library from an existing database and want to avoid UUID conflicts.
    pub fn dontusethis_randomize_library_uuid(&mut self) -> Result<(), CalibreError> {
        let conn = establish_connection(&self.validated_library_path.database_path);
        conn.map(|mut c| {
            diesel::sql_query("UPDATE library_id SET uuid = uuid4()")
                .execute(&mut c)
                .expect("Failed to set new UUID");
        })
        .map_err(|_| CalibreError::DatabaseError)
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

fn combined_author_sort(author_list: &Vec<Author>) -> String {
    author_list
        .iter()
        .map(|author| author.sortable_name())
        .collect::<Vec<String>>()
        .join(" & ")
}

fn default_pubdate() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("0101-01-01T00:00:00+00:00")
        .unwrap()
        .with_timezone(&Utc)
}

fn update_book_data_for_path(path: &PathBuf) -> UpdateBookData {
    let path_as_string = path.to_str().unwrap().to_string();
    UpdateBookData {
        author_sort: None,
        title: None,
        timestamp: None,
        pubdate: None,
        series_index: None,
        path: Some(path_as_string),
        flags: None,
        has_cover: None,
        last_modified: None,
    }
}

fn gen_book_file_name(book_title: &String, author_name: &String) -> String {
    sanitise(&deunicode(
        &"{title} - {author}"
            .replace("{title}", book_title)
            .replace("{author}", author_name),
    ))
}

fn gen_book_folder_name(book_id: i32) -> String {
    book_id.to_string()
}

#[derive(Default)]
struct Metadata<'a> {
    author_list: &'a [Author],
    publisher: &'a [Publisher],
    identifiers: &'a [Identifier],
    language: Option<&'a Language>,
    tags: &'a [Tag],
    rating: Option<&'a Rating>,
}

struct MetadataOpf<'a> {
    book: &'a Book,
    metadata: &'a Metadata<'a>,
}

impl<'a> MetadataOpf<'a> {
    pub fn new(book: &'a Book, metadata: &'a Metadata) -> Self {
        Self { book, metadata }
    }

    pub fn format(&self) -> Result<String, ()> {
        let book_custom_author_sort = self
            .book
            .author_sort
            .clone()
            .unwrap_or_else(|| self.get_author_sort_string(self.metadata.author_list));

        let authors_string =
            self.get_authors_string(self.metadata.author_list, &book_custom_author_sort);
        let publisher_string = self.get_publisher_string(self.metadata.publisher);
        let identifiers_string = self.get_identifiers_string(self.metadata.identifiers);
        let pubdate_string = self.get_pubdate_string(self.book.pubdate.as_ref());
        let language_string = self.get_language_string(self.metadata.language);
        let tags_string = self.get_tags_string(self.metadata.tags);
        let link_map_string = self.get_link_map_string(self.metadata.author_list);
        let rating_string = self.get_rating_string(self.metadata.rating);

        Ok(self.format_metadata_opf(
            self.book,
            &authors_string,
            &publisher_string,
            &identifiers_string,
            &pubdate_string,
            &language_string,
            &tags_string,
            &link_map_string,
            &rating_string,
        ))
    }

    fn get_author_sort_string(&self, author_list: &[Author]) -> String {
        if author_list.is_empty() {
            return String::new();
        }

        author_list
            .iter()
            .map(|author| author.name.clone())
            .collect::<Vec<String>>()
            .join(", ")
    }

    fn get_authors_string(
        &self,
        author_list: &[Author],
        book_custom_author_sort: &String,
    ) -> String {
        if author_list.is_empty() {
            return String::new();
        }

        author_list
            .iter()
            .map(|author| {
                format!(
                    "<dc:creator opf:file-as=\"{sortable}\" opf:role=\"aut\">{author}</dc:creator>",
                    sortable = book_custom_author_sort.as_str(),
                    author = author.name
                )
            })
            .collect::<String>()
    }

    fn get_publisher_string(&self, publisher: &[Publisher]) -> String {
        if publisher.is_empty() {
            return String::new();
        }

        let combined_publishers = publisher
            .iter()
            .map(|publ| publ.name.clone())
            .collect::<Vec<String>>()
            .join("&amp;");

        format!("<dc:publisher>{}</dc:publisher>", combined_publishers)
    }

    fn get_pubdate_string(&self, pubdate: Option<&DateTime<Utc>>) -> String {
        match pubdate {
            Some(date) => format!("<dc:date>{}</dc:date>", date.to_rfc3339()),
            None => "<dc:date>0101-01-01T00:00:00+00:00</dc:date>".to_string(),
        }
    }

    fn get_identifiers_string(&self, identifiers: &[Identifier]) -> String {
        if identifiers.is_empty() {
            return String::new();
        }

        identifiers
            .iter()
            .map(|identifier| {
                format!(
                    "<dc:identifier opf:scheme=\"{}\">{}</dc:identifier>",
                    identifier.type_, identifier.val
                )
            })
            .collect::<String>()
    }

    fn get_language_string(&self, language: Option<&Language>) -> String {
        match language {
            Some(lang) => format!("<dc:language>{}</dc:language>", lang.lang_code),
            None => String::new(),
        }
    }

    fn get_tags_string(&self, tags: &[Tag]) -> String {
        const INDENT: usize = 8;

        if tags.is_empty() {
            return String::new();
        }

        tags.iter()
            .map(|tag| {
                format!(
                    "{}<dc:subject>{}</dc:subject>",
                    " ".repeat(INDENT),
                    tag.name
                )
            })
            .collect::<Vec<String>>()
            .join("\n")
    }

    fn get_link_map_string(&self, author_list: &[Author]) -> String {
        if author_list.is_empty() {
            return String::new();
        }

        let mut authors_map = std::collections::HashMap::new();

        for author in author_list {
            authors_map.insert(author.name.clone(), author.link.clone());
        }

        let mut link_maps = std::collections::HashMap::new();
        link_maps.insert("authors".to_string(), authors_map);

        let json_string = serde_json::to_string(&link_maps).unwrap_or_default();
        let formatted_json = json_string.replace("\":", "\": ");
        let escaped_content = formatted_json.replace("\"", "&quot;");

        format!(
            "<meta name=\"calibre:link_maps\" content=\"{}\"/>",
            escaped_content
        )
    }

    fn get_rating_string(&self, rating: Option<&Rating>) -> String {
        match rating {
            Some(r) => format!("<meta name=\"calibre:rating\" content=\"{}\"/>", r.rating),
            None => String::new(),
        }
    }

    fn format_metadata_opf(
        &self,
        book: &Book,
        authors_string: &String,
        publisher_string: &String,
        identifiers_string: &String,
        pub_date_string: &String,
        language_string: &String,
        tags_string: &String,
        link_map_string: &String,
        rating_string: &String,
    ) -> String {
        let raw_xml = format!(
            r#"<?xml version='1.0' encoding='utf-8'?>
<package xmlns="http://www.idpf.org/2007/opf" unique-identifier="uuid_id" version="2.0">
    <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
        <dc:identifier opf:scheme="calibre" id="calibre_id">{calibre_id}</dc:identifier>
        <dc:identifier opf:scheme="uuid" id="uuid_id">{calibre_uuid}</dc:identifier>
        <dc:title>{book_title}</dc:title>
        {authors}
        {publisher}
        {identifiers}
        <dc:contributor opf:file-as="calibre" opf:role="bkp">EhArchive (0.1.0) [https://github.com/AyaseFile/EhArchive]</dc:contributor>
        {pub_date}
        {language_iso_639_3}
{tags}
        {link_map}
        {rating}
        <meta name="calibre:timestamp" content="{now}"/>
        <meta name="calibre:title_sort" content="{book_title_sortable}"/>
    </metadata>
    <guide>
        <reference type="cover" title="Cover" href="cover.jpg"/>
    </guide>
</package>"#,
            calibre_id = book.id,
            calibre_uuid = &book.uuid.clone().unwrap_or("".to_string()).as_str(),
            book_title = book.title,
            authors = authors_string,
            publisher = publisher_string,
            identifiers = identifiers_string,
            pub_date = pub_date_string,
            language_iso_639_3 = language_string,
            tags = tags_string,
            link_map = link_map_string,
            rating = rating_string,
            now = book.timestamp.unwrap().format("%Y-%m-%dT%H:%M:%S.%6f%:z"),
            book_title_sortable = &book.sort.clone().unwrap_or("".to_string()).as_str()
        );
        raw_xml
            .lines()
            .filter(|line| {
                !line.trim().is_empty()
                    || line.trim().starts_with("<")
                    || line.trim().ends_with(">")
            })
            .collect::<Vec<&str>>()
            .join("\n")
    }
}

#[derive(Debug)]
enum ClientError {
    GenericError,
}
impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
impl std::error::Error for ClientError {}

/// Create a new directory at a library-relative path.
/// Convenience function to avoid having absolute paths for files everywhere.
fn library_relative_mkdir(valid_db_path: &ValidDbPath, rel_path: PathBuf) -> io::Result<()> {
    let complete_path = Path::new(&valid_db_path.library_path).join(rel_path);

    match complete_path.exists() {
        true => Ok(()),
        _ => fs::create_dir_all(complete_path),
    }
}

/// Copy a file from an absolute path to a library-relative path, using a ValidDbPath.
/// Convenience function to avoid having to create the abs. path yourself.
fn library_relative_copy_file(
    valid_db_path: &ValidDbPath,
    source_abs: &Path,
    dest_rel: &Path,
) -> io::Result<()> {
    match source_abs.exists() {
        true => {
            let complete_dest_path = Path::new(&valid_db_path.library_path).join(dest_rel);
            fs::copy(source_abs, complete_dest_path).map(|_| ())
        }
        false => Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Source file does not exist: {:?}", source_abs),
        )),
    }
}

/// Write `contents` to a library-relative file path.
/// Convenience function to avoid having to create the absolute path.
fn library_relative_write_file(
    valid_db_path: &ValidDbPath,
    rel_path: &Path,
    contents: &[u8],
) -> io::Result<()> {
    let complete_path = Path::new(&valid_db_path.library_path).join(rel_path);
    fs::write(complete_path, contents)
}
