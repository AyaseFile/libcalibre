pub mod add_book;
pub mod replace_book;
pub mod update_book;
pub mod utils;

pub use utils::*;

use crate::dtos::author::UpdateAuthorDto;
use crate::entities::language::Language;
use crate::entities::rating::Rating;
use crate::entities::tag::Tag;
use crate::Book;
use crate::Publisher;
use crate::UpsertBookIdentifier;
use chrono::DateTime;
use chrono::Utc;

use std::error::Error;

use diesel::RunQueryDsl;

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
    pub validated_library_path: ValidDbPath,
    pub client_v2: ClientV2,
}

impl CalibreClient {
    pub fn new(db_path: ValidDbPath) -> CalibreClient {
        CalibreClient {
            validated_library_path: db_path.clone(),
            client_v2: ClientV2::new(db_path),
        }
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

    pub fn get_all_authors(&mut self) -> Result<Vec<Author>, Box<dyn Error>> {
        Ok(self.client_v2.authors().get_all_authors().unwrap())
    }

    pub fn replace_author_with_translation(
        &mut self,
        author_id: i32,
        translation: &str,
    ) -> Result<(), Box<dyn Error>> {
        self.client_v2
            .authors()
            .replace_with_translation(author_id, translation)
            .unwrap();
        Ok(())
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

    pub fn get_all_publishers(&mut self) -> Result<Vec<Publisher>, Box<dyn Error>> {
        Ok(self.client_v2.publishers().get_all_publishers().unwrap())
    }

    pub fn replace_publisher_with_translation(
        &mut self,
        publisher_id: i32,
        translation: &str,
    ) -> Result<(), Box<dyn Error>> {
        self.client_v2
            .publishers()
            .replace_with_translation(publisher_id, translation)
            .unwrap();
        Ok(())
    }

    pub fn get_all_tags(&mut self) -> Result<Vec<Tag>, Box<dyn Error>> {
        Ok(self.client_v2.tags().get_all_tags().unwrap())
    }

    pub fn replace_tag_with_translation(
        &mut self,
        tag_id: i32,
        translation: &str,
    ) -> Result<(), Box<dyn Error>> {
        self.client_v2
            .tags()
            .replace_with_translation(tag_id, translation)
            .unwrap();
        Ok(())
    }
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
