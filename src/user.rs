use chrono::Utc;
use derive_getters::Getters;
use mongodb::bson::Bson;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display, Formatter};
use std::ops::Add;
use std::str::FromStr;
use teloxide::prelude::{ChatId, UserId};
use teloxide::types::Recipient;

#[serde_with_macros::skip_serializing_none]
#[derive(Default, Debug, PartialEq, Clone, Getters, Serialize, Deserialize)]
pub struct User {
    id: MyUserId,
    join_date: chrono::DateTime<Utc>,
    first_name: String,
    last_name: Option<String>,
    username: String,
    language_code: Option<String>
}

impl User {
    pub fn new(user: &teloxide::types::User) -> Self {
        Self {
            id: user.id.into(),
            join_date: Utc::now().add(chrono::Duration::hours(3)),
            first_name: user.first_name.clone(),
            last_name: user.last_name.clone(),
            username: user.username.clone().unwrap(),
            language_code: user.language_code.clone(),
        }
    }
}

#[derive(Default, Debug, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub struct MyUserId(pub u64);

impl From<MyUserId> for Bson {
    fn from(value: MyUserId) -> Self {
        Bson::Int64(value.0 as i64)
    }
}

impl From<MyUserId> for ChatId {
    fn from(value: MyUserId) -> Self {
        Self(value.0 as i64)
    }
}

impl From<UserId> for MyUserId {
    fn from(value: UserId) -> Self {
        Self(value.0)
    }
}

impl From<ChatId> for MyUserId {
    fn from(value: ChatId) -> Self {
        Self(value.0 as u64)
    }
}

impl From<u64> for MyUserId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl FromStr for MyUserId {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.parse::<u64>()?))
    }
}

impl From<MyUserId> for Recipient {
    fn from(value: MyUserId) -> Self {
        Self::Id(ChatId(value.0 as i64))
    }
}

impl Display for MyUserId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
