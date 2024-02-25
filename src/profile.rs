use crate::common::keyboard::text;
use crate::user::MyUserId;

use derive_getters::Getters;
use enum_iterator::Sequence;
use mongodb::bson::{doc, Bson, Document};
use serde::{Deserialize, Serialize};

use std::fmt::{Display, Formatter};

use std::str::FromStr;
use teloxide::types::{InputFile, InputMedia, InputMediaPhoto, ParseMode};

pub type Age = i64;

pub const MAX_NAME_LENGTH: usize = 25;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct View {
    from: MyUserId,
    to: MyUserId,
    timestamp: mongodb::bson::DateTime,
    liked: bool,
}

impl View {
    pub fn new(from: impl Into<MyUserId>, to: impl Into<MyUserId>, liked: bool) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            timestamp: mongodb::bson::DateTime::now(),
            liked,
        }
    }
}

impl From<View> for Bson {
    fn from(value: View) -> Self {
        mongodb::bson::to_bson(&value).unwrap()
    }
}

impl Default for View {
    fn default() -> Self {
        Self {
            from: MyUserId::default(),
            to: MyUserId::default(),
            timestamp: mongodb::bson::DateTime::now(),
            liked: false,
        }
    }
}

#[derive(Debug, Default, Clone, Sequence)]
pub enum ProfileBuildingState {
    #[default]
    Name,
    Age,
    Location,
    Sex,
    MeetingPreferences,
    HearingLevel,
    Description,
    Photo,
}

#[derive(Debug, Clone, Default)]
pub struct ProfileBuilder {
    pub id: MyUserId,
    pub photos: Vec<PhotoId>,
    pub name: Option<String>,
    pub age: Option<Age>,
    pub sex: Option<Sex>,
    pub want_to_meet: Option<Sex>,
    pub hearing_level: Option<HearingLevel>,
    pub location: Option<Location>,
    pub description: Option<String>,
}

impl ProfileBuilder {
    pub fn build(self) -> Profile {
        Profile {
            id: self.id,
            photos: self.photos,
            name: self.name.unwrap_or(String::from("Не указано")),
            age: self.age.unwrap_or_default(),
            sex: self.sex.unwrap_or_default(),
            hearing_level: self.hearing_level.unwrap_or_default(),
            location: self.location.unwrap_or_default(),
            description: self.description,
            settings: Settings {
                show_up_in_search: true,
                search_options: SearchOptions {
                    sex: self.want_to_meet,
                    ..Default::default()
                }
            },
        }
    }
}

#[serde_with_macros::skip_serializing_none]
#[derive(Debug, Clone, Default, Getters, Serialize, Deserialize)]
pub struct Profile {
    id: MyUserId,
    photos: Vec<PhotoId>,
    name: String,
    age: Age,
    sex: Sex,
    hearing_level: HearingLevel,
    location: Location,
    description: Option<String>,
    settings: Settings,
}

impl Profile {
    pub fn builder(user_id: impl Into<MyUserId>) -> ProfileBuilder {
        ProfileBuilder {
            id: user_id.into(),
            ..Default::default()
        }
    }

    pub fn search_filter(&self, viewed_profiles: &[View]) -> Document {
        let viewed_profiles: Vec<i64> = viewed_profiles.iter().map(|el| el.to.0 as i64).collect();

        let mut options = vec![
            doc! { "id": { "$ne": self.id.0 as i64 } },
            doc! { "settings.show_up_in_search": true },
            doc! { "location.actual": &self.location.actual },
            doc! { "$nor": [ { "id": { "$in": viewed_profiles } } ] },
            doc! {
                "$or": [
                    { "settings.search_options.age": { "$exists": false } },
                    { "settings.search_options.age.greatest": { "$gte": self.age } },
                    { "settings.search_options.age.lowest": { "$lte": self.age } }
                ]
            },
            doc! {
                "$or": [
                    { "settings.search_options.sex": { "$exists": false } },
                    { "settings.search_options.sex": self.sex }
                ]
            },
            doc! {
                "$or": [
                    { "settings.search_options.hearing_level": { "$exists": false } },
                    { "settings.search_options.hearing_level": { "$in": [ self.hearing_level ] } }
                ]
            },
        ];

        if let Some(af) = self.settings().search_options().age.as_ref() {
            options.push(doc! { "age": { "$gte": af.lowest, "$lte": af.greatest } })
        }

        if let Some(sex) = self.settings().search_options().sex.as_ref() {
            options.push(doc! { "sex": sex })
        }

        if let Some(hl) = self.settings().search_options().hearing_level.as_ref() {
            options.push(doc! { "hearing_level": { "$in": hl } })
        }

        doc! { "$and": options }
    }

    pub fn to_mediagroup(&self) -> Vec<InputMedia> {
        let mut media_group = Vec::with_capacity(self.photos().len());

        media_group.push(InputMedia::Photo(
            InputMediaPhoto::new(InputFile::file_id(&self.photos().first().unwrap().0))
                .parse_mode(ParseMode::Html)
                .caption(self.to_caption()),
        ));

        for id in self.photos()[1..].iter() {
            media_group.push(InputMedia::Photo(InputMediaPhoto::new(InputFile::file_id(
                &id.0,
            ))));
        }

        media_group
    }

    fn to_caption(&self) -> String {
        let description = match self.description() {
            Some(text) => format!("\n\n📝 {text}"),
            None => String::new(),
        };

        let hearing_level = match (self.sex, self.hearing_level) {
            (Sex::Male, HearingLevel::CompletelyDeaf) => text::DEAF_BOY,
            (Sex::Male, HearingLevel::HearingImpaired) => text::HEARING_IMPAIRED_BOY,
            (Sex::Male, HearingLevel::Hearing) => text::HEARING_BOY,
            (Sex::Female, HearingLevel::CompletelyDeaf) => text::DEAF_GIRL,
            (Sex::Female, HearingLevel::HearingImpaired) => text::HEARING_IMPAIRED_GIRL,
            (Sex::Female, HearingLevel::Hearing) => text::HEARING_GIRL,
        };

        format!(
            "{}, {}, {}, {hearing_level}{description}",
            self.name(),
            self.age(),
            self.location(),
        )
    }
}

#[serde_with_macros::skip_serializing_none]
#[derive(Debug, PartialEq, Clone, Getters, Serialize, Deserialize)]
pub struct Like {
    from: MyUserId,
    to: MyUserId,
    timestamp: mongodb::bson::DateTime,
    message: Option<String>,
}

impl Like {
    pub fn new(
        from: impl Into<MyUserId>,
        to: impl Into<MyUserId>,
        message: impl Into<Option<String>>,
    ) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            timestamp: mongodb::bson::DateTime::now(),
            message: message.into(),
        }
    }
}

impl From<Like> for Bson {
    fn from(value: Like) -> Self {
        mongodb::bson::to_bson(&value).unwrap()
    }
}

impl Default for Like {
    fn default() -> Self {
        Self {
            from: MyUserId::default(),
            to: MyUserId::default(),
            timestamp: mongodb::bson::DateTime::now(),
            message: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum Sex {
    #[default]
    Male,
    Female,
}

impl Display for Sex {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Male => write!(f, "Male"),
            Self::Female => write!(f, "Female"),
        }
    }
}

impl From<Sex> for Bson {
    fn from(value: Sex) -> Self {
        Self::String(value.to_string())
    }
}

impl FromStr for Sex {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            text::BOY => Ok(Self::Male),
            text::GIRL => Ok(Self::Female),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum HearingLevel {
    #[default]
    CompletelyDeaf,
    HearingImpaired,
    Hearing,
}

impl Display for HearingLevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Hearing => write!(f, "Hearing"),
            Self::HearingImpaired => write!(f, "HearingImpaired"),
            Self::CompletelyDeaf => write!(f, "CompletelyDeaf"),
        }
    }
}

impl From<HearingLevel> for Bson {
    fn from(value: HearingLevel) -> Self {
        mongodb::bson::to_bson(&value).unwrap()
    }
}

impl FromStr for HearingLevel {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            text::DEAF_GIRL | text::DEAF_BOY => Ok(Self::CompletelyDeaf),
            text::HEARING_IMPAIRED_GIRL | text::HEARING_IMPAIRED_BOY => Ok(Self::HearingImpaired),
            text::HEARING_GIRL | text::HEARING_BOY => Ok(Self::Hearing),

            "CompletelyDeaf" => Ok(Self::CompletelyDeaf),
            "HearingImpaired" => Ok(Self::HearingImpaired),
            "Hearing" => Ok(Self::Hearing),
            _ => Err(()),
        }
    }
}

#[serde_with_macros::skip_serializing_none]
#[derive(Debug, Clone, Default, Getters, Serialize, Deserialize)]
pub struct Settings {
    search_options: SearchOptions,
    show_up_in_search: bool,
}

#[serde_with_macros::skip_serializing_none]
#[derive(Debug, Clone, Default, Getters, Serialize, Deserialize)]
pub struct SearchOptions {
    age: Option<AgeFilter>,
    sex: Option<Sex>,
    distance: Option<DistanceFilter>,
    hearing_level: Option<Vec<HearingLevel>>,
}

impl From<SearchOptions> for Bson {
    fn from(value: SearchOptions) -> Self {
        mongodb::bson::to_bson(&value).unwrap()
    }
}

#[serde_with_macros::skip_serializing_none]
#[derive(Debug, Clone, PartialEq, Getters, Serialize, Deserialize)]
pub struct Location {
    displayed: String,
    actual: String,
    coordinates: Option<Coordinates>,
}

impl Location {
    pub fn new(
        displayed: impl Into<String>,
        actual: impl Into<String>,
        coords: impl Into<Option<Coordinates>>,
    ) -> Self {
        Self {
            displayed: displayed.into(),
            actual: actual.into(),
            coordinates: coords.into(),
        }
    }
}

impl From<Location> for Bson {
    fn from(value: Location) -> Self {
        mongodb::bson::to_bson(&value).unwrap()
    }
}

impl Display for Location {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.displayed)
    }
}

impl Default for Location {
    fn default() -> Self {
        Self {
            displayed: String::from("Москва"),
            actual: String::from("Москва"),
            coordinates: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Getters, Serialize, Deserialize)]
pub struct Coordinates {
    longitude: f64,
    latitude: f64,
}

#[derive(Debug, PartialEq, Clone, Default, Getters, Serialize, Deserialize)]
pub struct AgeFilter {
    pub(crate) greatest: Age,
    pub(crate) lowest: Age,
}

#[derive(Debug, PartialEq, Clone, Default, Getters, Serialize, Deserialize)]
pub struct DistanceFilter {
    max_meters: u64,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct PhotoId(pub String);

pub fn validate_name(n: &str) -> Result<(), NameValidationError> {
    let length = n.len();

    if length > MAX_NAME_LENGTH {
        return Err(NameValidationError::TooLong {
            name: n.to_owned(),
            length,
        });
    }

    Ok(())
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum NameValidationError {
    #[error("name `{name}` is too long. max is {MAX_NAME_LENGTH}, got {length}")]
    TooLong { name: String, length: usize },
}
