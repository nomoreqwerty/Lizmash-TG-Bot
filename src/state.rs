use super::*;
use crate::common::keyboard::*;
use crate::common::structs::{SearchData};
use crate::common::{conversation, keyboard, CheckForMatchResult};
use crate::database::Database;
use crate::defines::{BotDialogue, TeloxideResult};
use crate::maps::{FetchingError, Maps};
use crate::profile::{
    Age, HearingLevel, NameValidationError, PhotoId, ProfileBuilder, ProfileBuildingState, Sex,
};
use crate::user::{User};
use enum_iterator::Sequence;
use std::error::Error;
use std::str::FromStr;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{Location, MessageId, ParseMode, PhotoSize};

use thiserror::Error;
use crate::callback::ProfileField;

type CreatingProfileResult = Result<ProfileBuilder, CreatingProfileError>;

#[derive(Debug, Error)]
pub enum CreatingProfileError {
    #[error("teloxide error: {error:?}")]
    TeloxideError {
        #[from]
        error: Box<dyn Error + Send + Sync>,
    },

    #[error("unable to parse user reply. state: {state:?}, reply: `{user_reply}`")]
    UnableToParseUserReply {
        state: ProfileBuildingState,
        user_reply: String,
    },

    #[error("name validation error: {error:?}")]
    UnableToValidateName {
        #[from]
        error: NameValidationError,
    },
}

#[derive(Clone, Default, Debug)]
pub enum State {
    #[default]
    None,

    CreatingProfile {
        profile_builder: ProfileBuilder,
        state: ProfileBuildingState,
    },

    LookingAtProfiles {
        data: SearchData,
    },

    LookingAtProfilesWhoHaveLiked {
        data: SearchData,
    },

    EditingProfile {
        profile_field: ProfileField,
        callback_query: CallbackQuery
    },
}

/// Acting on the current viewing profile.
pub async fn act_on_profile(
    bot: Bot,
    db: Arc<Database>,
    dialogue: BotDialogue,
    msg: Message,
    data: SearchData,
) -> TeloxideResult {
    match msg.text() {
        Some(text::LIKE) => {
            match common::check_for_match(db.clone(), data.profile_id, msg.chat.id).await {
                CheckForMatchResult::Match => {
                    bot.send_message(msg.chat.id, "У вас взаимный лайк 👇")
                        .await?;
                    perform::match_likes(
                        bot.clone(),
                        db.clone(),
                        dialogue.clone(),
                        msg.chat.id,
                        &data,
                    )
                    .await?;

                    db.remove_like(data.profile_id, msg.chat.id).await;
                }
                CheckForMatchResult::DontMatch => {
                    perform::like_profile(bot.clone(), db.clone(), msg.chat.id, &data).await?;
                }
            }
        }
        Some(text::DISLIKE) => {
            perform::dislike_profile(db.clone(), msg.chat.id, &data).await?;
        }
        Some(text::MENU) => {
            perform::enter_menu(bot, dialogue, msg.chat.id).await?;
            return Ok(());
        }
        _ => {}
    }

    perform::send_new_suggestion(bot, db, dialogue, msg.chat.id, data).await?;

    Ok(())
}

pub async fn edit_profile(
    bot: Bot,
    db: Arc<Database>,
    maps: Arc<Maps>,
    dialogue: BotDialogue,
    msg: Message,
    (edit_kind, callback_query): (ProfileField, CallbackQuery),
) -> TeloxideResult {
    match (edit_kind, msg.text(), msg.photo()) {
        (ProfileField::Name, Some(new_name), _) => db.set_profile_name(msg.chat.id, new_name).await,
        (ProfileField::Age, Some(new_age), _) => {
            if let Ok(age) = new_age.parse() { db.set_profile_age(msg.chat.id, age).await }
        },
        (ProfileField::City, Some(new_city), _) => {
            let location = match maps.get_actual_city(new_city).await {
                Ok(loc) => loc,
                Err(FetchingError::CityNotFound { name }) => {
                    bot.parse_mode(ParseMode::Html).send_message(msg.chat.id, format!("🕵🏻‍♂️ Города <b>{name}</b> не существует")).await?;
                    return Ok(())
                }
                Err(error) => {
                    bot.send_message(msg.chat.id, "Неизвестная ошибка. Сообщите о ней разработчику: nomoreqwerty@tuta.io").await?;
                    panic!("unknown error: {error}");
                }
            };
            db.set_profile_location(msg.chat.id, location).await;
        }
        (ProfileField::Description, Some(new_description), _) => db.set_profile_description(msg.chat.id, new_description).await,
        (ProfileField::Photo, _, Some(photos)) => {
            let photos = [photos.last().unwrap().file.id.to_owned()];
            db.set_profile_picture(msg.chat.id, &photos).await;
        }
        _ => {}
    }

    for id in (msg.id.0)..(callback_query.message.as_ref().unwrap().id.0) {
        bot.delete_message(msg.chat.id, MessageId(id)).await?;
    }

    bot.delete_message(msg.chat.id, msg.id).await?;

    bot.edit_message_text(
        callback_query.from.id,
        callback_query.message.as_ref().unwrap().id,
        common::text::PROFILE_EDIT_MODE
    )
        .await?;

    bot.edit_message_reply_markup(
        callback_query.from.id,
        callback_query.message.as_ref().unwrap().id,
    )
        .reply_markup(EditProfile::keyboard())
        .await?;

    dialogue.reset().await?;

    Ok(())
}

pub async fn look_at_likes(
    bot: Bot,
    db: Arc<Database>,
    dialogue: BotDialogue,
    msg: Message,
    data: SearchData,
) -> TeloxideResult {
    match msg.text() {
        Some(text::LIKE) => {
            perform::match_likes(
                bot.clone(),
                db.clone(),
                dialogue.clone(),
                msg.chat.id,
                &data,
            )
            .await?
        }
        Some(text::DISLIKE) => {
            perform::dislike_profile(db.clone(), msg.chat.id, &data).await?;
        }
        Some(text::MENU) => {
            perform::enter_menu(bot, dialogue, msg.chat.id).await?;
            return Ok(());
        }
        _ => {}
    }

    db.remove_like(data.profile_id, msg.chat.id).await;
    perform::give_new_liked_profile(bot, db, dialogue, msg.chat.id, data).await?;

    Ok(())
}

pub async fn handle_message(
    bot: Bot,
    db: Arc<Database>,
    dialogue: BotDialogue,
    msg: Message,
) -> TeloxideResult {
    match msg.text() {
        Some(text::WATCH_PROFILES) => {
            perform::start_looking_at_profiles(bot, db, dialogue, msg.chat.id).await?
        }
        Some(text::MY_PROFILE) => match db.get_profile(msg.chat.id).await {
            Some(ref profile) => {
                conversation::send_profile_with_keyboard_inline(
                    bot,
                    msg.chat.id,
                    profile,
                    EnterProfileEditingMode::keyboard(),
                )
                .await?
            }
            None => conversation::default_start(bot, msg.chat.id).await?,
        },
        Some(text::WHO_LIKES_ME) => {
            perform::start_looking_at_likes(bot, db, dialogue, msg.chat.id).await?
        }
        Some(text::MENU) | Some(text::LIKE) | Some(text::DISLIKE) => {
            bot.send_message(
                msg.chat.id,
                "🫠 По какой-то причине ваша предыдущая активность была утеряна",
            )
            .await?;
            perform::enter_menu(bot, dialogue, msg.chat.id).await?;
        }
        _ => {}
    }
    Ok(())
}

pub async fn build_profile(
    bot: Bot,
    db: Arc<Database>,
    maps: Arc<Maps>,
    msg: Message,
    dialogue: BotDialogue,
    data: (ProfileBuilder, ProfileBuildingState),
) -> TeloxideResult {
    let (mut profile_builder, state) = data;

    let dialogue_result = match (state.clone(), msg.text(), msg.location(), msg.photo()) {
        (ProfileBuildingState::Name, Some(text), _, _) => {
            catch_name(bot, profile_builder, text).await
        }
        (ProfileBuildingState::Age, Some(text), _, _) => {
            catch_age(bot, profile_builder, text).await
        }
        (ProfileBuildingState::Location, text, location, _) => {
            catch_location(bot, profile_builder, maps, text, location).await
        }
        (ProfileBuildingState::Sex, Some(text), _, _) => {
            catch_sex(bot, profile_builder, text).await
        }
        (ProfileBuildingState::MeetingPreferences, Some(text), _, _) => {
            catch_meeting_preferences(bot, profile_builder, text).await
        }
        (ProfileBuildingState::HearingLevel, Some(text), _, _) => {
            catch_hearing_level(bot, profile_builder, text).await
        }
        (ProfileBuildingState::Description, Some(text), _, _) => {
            catch_description(bot, profile_builder, text).await
        }
        (ProfileBuildingState::Photo, _, _, Some(photos)) => {
            profile_builder = catch_photo(profile_builder, photos).await.unwrap();

            common::finish_profile_creation(
                bot,
                db,
                &User::new(msg.from().unwrap()),
                profile_builder,
            )
            .await?;
            dialogue.reset().await?;
            return Ok(());
        }
        _ => return Ok(()),
    };

    if let Ok(profile_builder) = dialogue_result {
        dialogue
            .update(State::CreatingProfile {
                profile_builder,
                state: state.next().unwrap(),
            })
            .await?;
    }

    Ok(())
}

#[inline]
pub async fn catch_name(
    bot: Bot,
    mut profile_builder: ProfileBuilder,
    text: &str,
) -> CreatingProfileResult {
    match profile::validate_name(text) {
        Err(NameValidationError::TooLong { name, length }) => {
            bot
                .parse_mode(ParseMode::Html)
                .send_message(
                profile_builder.id,
                format!("Имя `{name}` слишком длинное {length}/<b>{}</b>", profile::MAX_NAME_LENGTH)
            ).await.unwrap();

            return Err(CreatingProfileError::UnableToValidateName {
                error: NameValidationError::TooLong { name, length },
            });
        }
        Ok(()) => profile_builder.name = Some(text.to_owned()),
    }

    bot.send_message(profile_builder.id, "Сколько тебе лет?")
        .await
        .unwrap();

    Ok(profile_builder)
}

#[inline]
pub async fn catch_age(
    bot: Bot,
    mut profile_builder: ProfileBuilder,
    text: &str,
) -> CreatingProfileResult {
    profile_builder.age = match text.parse::<Age>() {
        Ok(age) => Some(age),
        Err(_) => {
            return Err(CreatingProfileError::UnableToParseUserReply {
                state: ProfileBuildingState::Age,
                user_reply: text.to_owned(),
            })
        }
    };

    bot.send_message(profile_builder.id, "В каком городе ты живёшь?")
        //.reply_markup(RequestLocation::keyboard())
        .await
        .unwrap();

    Ok(profile_builder)
}

#[inline]
pub async fn catch_location(
    bot: Bot,
    mut profile_builder: ProfileBuilder,
    maps: Arc<Maps>,
    text: Option<&str>,
    location: Option<&Location>,
) -> CreatingProfileResult {
    profile_builder.location = match (text, location) {
        (Some(string), None) => Some(maps.get_actual_city(string).await.unwrap()),
        (None, Some(_loc)) => todo!(),
        (_, _) => unreachable!(),
    };

    bot.send_message(profile_builder.id, "Ты парень или девушка?")
        .reply_markup(SelectSex::keyboard())
        .await
        .unwrap();

    Ok(profile_builder)
}

#[inline]
pub async fn catch_sex(
    bot: Bot,
    mut profile_builder: ProfileBuilder,
    text: &str,
) -> CreatingProfileResult {
    profile_builder.sex = match Sex::from_str(text) {
        Ok(sex) => Some(sex),
        Err(_) => {
            return Err(CreatingProfileError::UnableToParseUserReply {
                state: ProfileBuildingState::Sex,
                user_reply: text.to_owned(),
            })
        }
    };

    bot.send_message(profile_builder.id, "Кого ты хочешь встретить?")
        .reply_markup(SelectWantToMeet::keyboard())
        .await
        .unwrap();

    Ok(profile_builder)
}

#[inline]
pub async fn catch_meeting_preferences(
    bot: Bot,
    mut profile_builder: ProfileBuilder,
    text: &str,
) -> CreatingProfileResult {
    profile_builder.want_to_meet = match text {
        "Парня" => Some(Sex::Male),
        "Девушку" => Some(Sex::Female),
        "Без разницы" => None,
        _ => return Ok(profile_builder),
    };

    bot.send_message(profile_builder.id, "Какой у тебя уровень слуха?")
        .reply_markup(keyboard::HearingLevel::keyboard(
            *profile_builder.sex.as_ref().unwrap(),
        ))
        .await
        .unwrap();

    Ok(profile_builder)
}

#[inline]
pub async fn catch_hearing_level(
    bot: Bot,
    mut profile_builder: ProfileBuilder,
    text: &str,
) -> CreatingProfileResult {
    profile_builder.hearing_level = match HearingLevel::from_str(text) {
        Ok(hearing_level) => Some(hearing_level),
        Err(_) => {
            return Err(CreatingProfileError::UnableToParseUserReply {
                state: ProfileBuildingState::HearingLevel,
                user_reply: text.to_owned(),
            })
        }
    };

    bot.send_message(profile_builder.id, "Добавь описание для своей анкеты")
        .reply_markup(LeaveEmptyDescription::keyboard())
        .await
        .unwrap();

    Ok(profile_builder)
}

#[inline]
pub async fn catch_description(
    bot: Bot,
    mut profile_builder: ProfileBuilder,
    text: &str,
) -> CreatingProfileResult {
    profile_builder.description = match text {
        text::LEAVE_EMPTY => None,
        string => Some(string.to_owned()),
    };

    bot.send_message(
        profile_builder.id,
        "Отправь свою фотографию, чтобы мы знали, как ты выглядишь",
    ).reply_markup(teloxide::types::KeyboardRemove::new())
    .await
    .unwrap();

    Ok(profile_builder)
}

#[inline]
pub async fn catch_photo(
    mut profile_builder: ProfileBuilder,
    photos: &[PhotoSize],
) -> CreatingProfileResult {
    profile_builder
        .photos
        .push(PhotoId(photos.last().unwrap().file.id.to_owned()));
    Ok(profile_builder)
}

