use std::fmt::{Display, Formatter};
use crate::database::Database;
use crate::defines::{BotDialogue, TeloxideResult};
use crate::error::ParseCallbackDataError;
use std::str::FromStr;
use std::sync::Arc;
use teloxide::prelude::*;
use crate::perform;
use crate::profile::HearingLevel;

const EPEM: &str = "EPEM";
const EPD: &str = "EPD";
const SHR: &str = "SHR";
const LED: &str = "LED";
const FED: &str = "FED";

const SEP: &str = ":";

/// EPEM    - Enter Profile Edit Mode
///
/// EPD     - Edit Profile Data
///
/// SHR     - Set Hearing Level
///
/// LED     - Leave Empty Description
///
/// FED     - Finish Editing Profile
#[allow(clippy::upper_case_acronyms)]
pub enum CallbackData {
    EPEM,
    EPD { profile_field: ProfileField },
    SHR { hearing_level: HearingLevel },
    LED,
    FED,
}

impl CallbackData {
    fn from_epd(data: &[&str]) -> Self {
        Self::EPD {
            profile_field: ProfileField::from_str(data[0]).unwrap(),
        }
    }

    fn from_shr(data: &[&str]) -> Self {
        Self::SHR {
            hearing_level: HearingLevel::from_str(data[0]).unwrap(),
        }
    }
}

impl From<CallbackData> for String {
    fn from(value: CallbackData) -> Self {
        match value {
            CallbackData::EPEM => String::from(EPEM),
            CallbackData::EPD { profile_field } => format!("{EPD}{0}{profile_field}", SEP),
            CallbackData::SHR { hearing_level } => format!("{SHR}{0}{hearing_level}", SEP),
            CallbackData::LED => String::from(LED),
            CallbackData::FED => String::from(FED)
        }
    }
}

impl FromStr for CallbackData {
    type Err = ParseCallbackDataError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let split: Vec<&str> = s.split(SEP).collect();
        match split[0] {
            EPEM => Ok(Self::EPEM),
            EPD => Ok(Self::from_epd(&split[1..])),
            SHR => Ok(Self::from_shr(&split[1..])),
            LED => Ok(Self::LED),
            FED => Ok(Self::FED),
            _ => Err(ParseCallbackDataError::UnknownCallbackCode {
                code: split[0].to_owned(),
            }),
        }
    }
}

pub async fn handle(
    bot: Bot,
    q: CallbackQuery,
    db: Arc<Database>,
    dialogue: BotDialogue,
) -> TeloxideResult {
    bot.answer_callback_query(&q.id).await?;

    let callback_data = match q.data {
        Some(ref data) => CallbackData::from_str(data.as_str()).unwrap(),
        None => return Ok(()),
    };

    match callback_data {
        CallbackData::EPEM => perform::enter_profile_editing_mode(bot, q).await?,
        CallbackData::EPD { profile_field } => {
            perform::set_profile_editing_handler(bot, db, q, dialogue, profile_field).await?;
        }
        CallbackData::SHR { hearing_level } => perform::set_hearing_level(bot, db, q, hearing_level).await?,
        CallbackData::LED => perform::leave_empty_description(bot, db, q).await?,
        CallbackData::FED => perform::finish_editing_profile(bot, db, dialogue, q).await?,
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub enum ProfileField {
    Name,
    Age,
    City,
    HearingLevel,
    Description,
    Photo,
}

impl ProfileField {
    pub fn to_str(&self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Age => "Age",
            Self::City => "City",
            Self::HearingLevel => "HearingLevel",
            Self::Description => "Description",
            Self::Photo => "Photo",
        }
    }
}

impl Display for ProfileField {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_str())
    }
}

impl FromStr for ProfileField {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Name" => Ok(Self::Name),
            "Age" => Ok(Self::Age),
            "City" => Ok(Self::City),
            "HearingLevel" => Ok(Self::HearingLevel),
            "Description" => Ok(Self::Description),
            "Photo" => Ok(Self::Photo),
            _ => Err(())
        }
    }
}
