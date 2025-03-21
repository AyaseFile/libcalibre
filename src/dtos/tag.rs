use crate::entities::tag::NewTag;

#[derive(Clone)]
pub struct NewTagDto {
    pub name: String,
}

impl TryFrom<NewTagDto> for NewTag {
    type Error = ();

    fn try_from(dto: NewTagDto) -> Result<Self, Self::Error> {
        Ok(Self { name: dto.name })
    }
}
