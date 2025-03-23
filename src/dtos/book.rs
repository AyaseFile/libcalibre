use chrono::{DateTime, Utc};

use crate::entities::book::{NewBook, UpdateBookData};

#[derive(Clone)]
pub struct NewBookDto {
    pub title: String,
    pub timestamp: Option<DateTime<Utc>>,
    pub pubdate: Option<DateTime<Utc>>,
    pub series_index: f32,
    pub flags: i32,
    pub has_cover: Option<bool>,
}

pub struct UpdateBookDto {
    pub author_sort: Option<String>,
    pub title: Option<String>,
    pub timestamp: Option<DateTime<Utc>>,
    pub pubdate: Option<DateTime<Utc>>,
    pub series_index: Option<f32>,
    pub path: Option<String>,
    pub flags: Option<i32>,
    pub has_cover: Option<bool>,
    pub is_read: Option<bool>,
    pub last_modified: Option<DateTime<Utc>>,
}

impl UpdateBookDto {
    pub fn default() -> UpdateBookDto {
        Self {
            author_sort: None,
            title: None,
            timestamp: None,
            pubdate: None,
            series_index: None,
            path: None,
            flags: None,
            has_cover: None,
            is_read: None,
            last_modified: None,
        }
    }
}

impl TryFrom<NewBookDto> for NewBook {
    type Error = ();

    fn try_from(dto: NewBookDto) -> Result<Self, Self::Error> {
        Ok(Self {
            title: dto.title,
            timestamp: dto.timestamp,
            pubdate: dto.pubdate,
            series_index: dto.series_index,
            flags: dto.flags,
            has_cover: dto.has_cover,
        })
    }
}

impl TryFrom<UpdateBookDto> for UpdateBookData {
    type Error = ();

    fn try_from(dto: UpdateBookDto) -> Result<Self, Self::Error> {
        Ok(Self {
            author_sort: dto.author_sort,
            title: dto.title,
            timestamp: dto.timestamp,
            pubdate: dto.pubdate,
            series_index: dto.series_index,
            path: dto.path,
            flags: dto.flags,
            has_cover: dto.has_cover,
            last_modified: dto.last_modified,
        })
    }
}
