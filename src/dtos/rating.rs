use crate::entities::rating::NewRating;

#[derive(Clone)]
pub struct NewRatingDto {
    pub rating: i32,
}

impl TryFrom<NewRatingDto> for NewRating {
    type Error = ();

    fn try_from(dto: NewRatingDto) -> Result<Self, Self::Error> {
        Ok(Self { rating: dto.rating })
    }
}
