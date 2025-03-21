use crate::entities::publisher::NewPublisher;

#[derive(Clone)]
pub struct NewPublisherDto {
    pub name: String,
    pub sort: Option<String>,
}

impl TryFrom<NewPublisherDto> for NewPublisher {
    type Error = ();

    fn try_from(dto: NewPublisherDto) -> Result<Self, Self::Error> {
        Ok(Self {
            name: dto.name,
            sort: dto.sort,
        })
    }
}
