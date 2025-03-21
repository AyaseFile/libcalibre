use std::sync::Arc;
use std::sync::Mutex;

use diesel::prelude::*;

use crate::dtos::language::NewLanguageDto;
use crate::entities::language::NewLanguage;
use crate::Language;

pub struct LanguagesHandler {
    client: Arc<Mutex<SqliteConnection>>,
}

impl LanguagesHandler {
    pub(crate) fn new(client: Arc<Mutex<SqliteConnection>>) -> Self {
        Self { client }
    }

    pub fn create(&mut self, dto: NewLanguageDto) -> Result<Language, ()> {
        use crate::schema::languages::dsl::languages;
        let new_language = NewLanguage::try_from(dto)?;
        let mut connection = self.client.lock().unwrap();

        diesel::insert_into(languages)
            .values(new_language)
            .returning(Language::as_returning())
            .get_result::<Language>(&mut *connection)
            .or(Err(()))
    }

    pub fn create_if_missing(&mut self, dto: NewLanguageDto) -> Result<Language, ()> {
        match self.find_by_lang_code(&dto.lang_code)? {
            Some(language) => Ok(language),
            _ => self.create(dto),
        }
    }

    pub fn find_by_lang_code(&mut self, search_lang_code: &str) -> Result<Option<Language>, ()> {
        use crate::schema::languages::dsl::{lang_code, languages};
        let mut connection = self.client.lock().unwrap();

        languages
            .filter(lang_code.eq(search_lang_code))
            .select(Language::as_select())
            .get_result::<Language>(&mut *connection)
            .optional()
            .or(Err(()))
    }
}
