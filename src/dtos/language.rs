use crate::entities::language::NewLanguage;

#[derive(Clone)]
pub struct NewLanguageDto {
    pub lang_code: String,
}

impl TryFrom<NewLanguageDto> for NewLanguage {
    type Error = ();

    fn try_from(dto: NewLanguageDto) -> Result<Self, Self::Error> {
        Ok(Self {
            lang_code: dto.lang_code,
        })
    }
}
