use crate::{dtos::author::NewAuthorDto, UpsertBookIdentifier};
use std::path::PathBuf;

use super::{
    book::{NewBookDto, UpdateBookDto},
    language::NewLanguageDto,
    publisher::NewPublisherDto,
    rating::NewRatingDto,
    tag::NewTagDto,
};

pub struct NewLibraryFileDto {
    pub path: PathBuf,
    //pub name: String,
    //pub size: i64,
    //pub mime_type: String,
}

pub struct UpdateLibraryEntryDto {
    pub book: UpdateBookDto,
    pub author_id_list: Option<Vec<String>>,
}

pub struct NewLibraryEntryDto {
    pub book: NewBookDto,
    pub authors: Vec<NewAuthorDto>,
    pub publishers: Vec<NewPublisherDto>,
    pub identifiers: Vec<UpsertBookIdentifier>,
    pub language: Option<NewLanguageDto>,
    pub tags: Vec<NewTagDto>,
    pub rating: Option<NewRatingDto>,
    pub files: Option<Vec<NewLibraryFileDto>>,
}
