use crate::common::keyboard::{MakeKeyboard, Menu};
use crate::database::*;
use crate::defines::{TeloxideResult};
use crate::profile::{Profile, ProfileBuilder};
use crate::user::{MyUserId, User};
use std::fmt::{Debug};
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::ParseMode;

pub async fn finish_profile_creation(
    bot: Bot,
    db: Arc<Database>,
    user: &User,
    profile_builder: ProfileBuilder,
) -> TeloxideResult {
    let profile = profile_builder.build();

    if db.get_user(*user.id()).await.is_none() {
        db.add_user(user).await;
    }

    db.add_profile(&profile).await;

    bot.send_message(*profile.id(), "Готово. Вот твоя анкета:")
        .reply_markup(Menu::keyboard())
        .await?;

    conversation::send_profile(bot, *profile.id(), &profile).await?;

    Ok(())
}

pub async fn next_suggestion(db: Arc<Database>, user_id: impl Into<MyUserId>) -> Option<Profile> {
    let user_id = user_id.into();

    let user_profile = db.get_profile(user_id).await.unwrap();
    let viewed_profiles = db.get_user_views(user_id).await;

    let searching_filter = user_profile.search_filter(&viewed_profiles);

    db.filter_profile(searching_filter).await
}

/// Returns [Profile] of a user who liked user with the given _user_id_
///
/// Returns _None_ if there is no record of the user's like in the database,
/// or if the user does not have a profile
pub async fn next_profile_who_have_liked(
    db: Arc<Database>,
    user_id: impl Into<MyUserId>,
) -> Option<Profile> {
    let like = db.get_like_to_user(user_id.into()).await?;
    db.get_profile(*like.from()).await
}

pub async fn check_for_match(
    db: Arc<Database>,
    from: impl Into<MyUserId>,
    to: impl Into<MyUserId>,
) -> CheckForMatchResult {
    let (from, to) = (from.into(), to.into());

    db.find_like(from, to)
        .await
        .map_or(CheckForMatchResult::DontMatch, |_| {
            CheckForMatchResult::Match
        })
}

pub mod text {
    pub const PROFILE_EDIT_MODE: &str = "✏ Редактирование анкеты";
}

#[derive(Debug, Default, Clone)]
pub enum CheckForMatchResult {
    Match,
    #[default]
    DontMatch,
}

pub mod structs {
    use super::*;

    #[derive(Clone, Debug)]
    pub struct SearchData {
        pub user_profile: Profile,
        pub profile_id: MyUserId,
    }
}

pub mod conversation {
    use super::*;
    
    use std::error::Error;
    use teloxide::types::InlineKeyboardMarkup;

    pub async fn bot_works_only_in_chats(
        bot: Bot,
        msg: Message,
    ) -> TeloxideResult {
        bot.send_message(msg.chat.id, "Бот работает только в личных сообщениях")
            .await?;
        Ok(())
    }

    pub async fn send_username_is_needed(bot: Bot, msg: Message) -> TeloxideResult {
        bot.parse_mode(ParseMode::Html)
            .send_message(
                msg.chat.id,
                "Сожалею, но для работы с ботом нужно иметь <b>имя пользователя</b>\n\n\
                  Установить его можно в настройках",
            )
            .await?;
        Ok(())
    }

    pub async fn send_menu(bot: Bot, user_id: impl Into<MyUserId>) -> TeloxideResult {
        bot.send_message(user_id.into(), "🏠 Меню")
            .reply_markup(Menu::keyboard())
            .await?;
        Ok(())
    }

    pub async fn default_start(
        bot: Bot,
        user_id: impl Into<MyUserId>,
    ) -> TeloxideResult {
        let user_id = user_id.into();

        bot.parse_mode(ParseMode::Html)
            .send_message(
                user_id,
                "<b>Привет</b> 👋\n\
                    \n\
                    Чтобы начать поиск, необходимо сначала создать анкету. Как тебя зовут?",
            )
            .await?;

        Ok(())
    }

    pub async fn send_profile_with_keyboard_inline(
        bot: Bot,
        user_id: impl Into<MyUserId>,
        profile: &Profile,
        keyboard: InlineKeyboardMarkup,
    ) -> TeloxideResult {
        let message = send_profile(bot.clone(), user_id.into(), profile).await?;

        bot.edit_message_reply_markup(message.chat.id, message.id)
            .reply_markup(keyboard)
            .await?;

        Ok(())
    }

    #[inline]
    pub async fn send_profile(
        bot: Bot,
        user_id: impl Into<MyUserId>,
        profile: &Profile,
    ) -> Result<Message, Box<dyn Error + Send + Sync>> {
        let message = bot
            .send_media_group(user_id.into(), profile.to_mediagroup())
            .await?
            .first()
            .unwrap()
            .clone();
        Ok(message)
    }

    #[inline]
    pub async fn send_no_suggestion(bot: Bot, user_id: impl Into<MyUserId>) -> TeloxideResult {
        bot.send_message(
            user_id.into(),
            "Анкет, удовлетворяющих твоим критериям поиска, не найдено",
        )
        .reply_markup(Menu::keyboard())
        .await?;
        Ok(())
    }

    #[inline]
    pub async fn send_likes_are_over_now_search(
        bot: Bot,
        user_id: impl Into<MyUserId>,
    ) -> TeloxideResult {
        bot.send_message(user_id.into(), "Лайки закончились, включен режим поиска")
            .await?;
        Ok(())
    }
}

pub mod keyboard {
    use crate::callback::{CallbackData, ProfileField};
    use crate::profile::Sex;
    
    use teloxide::types::{
        ButtonRequest, InlineKeyboardButton, InlineKeyboardMarkup, KeyboardButton, KeyboardMarkup,
    };
    use text::*;
    use crate::profile;

    pub mod text {
        pub const BOY: &str = "Парень";
        pub const GIRL: &str = "Девушка";
        pub const LOCATION: &str = "📍 Местоположение";
        pub const WANT_A_BOY: &str = "Парня";
        pub const WANT_A_GIRL: &str = "Девушку";
        pub const WHATEVER: &str = "Без разницы";
        pub const DEAF_GIRL: &str = "Глухая";
        pub const DEAF_BOY: &str = "Глухой";
        pub const HEARING_IMPAIRED_GIRL: &str = "Слабослышащая";
        pub const HEARING_IMPAIRED_BOY: &str = "Слабослышащий";
        pub const HEARING_GIRL: &str = "Слышащая";
        pub const HEARING_BOY: &str = "Слышащий";
        pub const LEAVE_EMPTY: &str = "Оставить пустым";
        pub const WATCH_PROFILES: &str = "🚀 Поиск";
        pub const MY_PROFILE: &str = "⭐ Профиль";
        pub const LIKE: &str = "❤️";
        pub const DISLIKE: &str = "👎";
        pub const MENU: &str = "🏠";
        pub const WHO_LIKES_ME: &str = "📩 Лайки";
        pub const EDIT: &str = "✏ Редактировать";
        pub const FINISH: &str = "Закончить";
        pub const EDIT_NAME: &str = "✒ Имя";
        pub const EDIT_AGE: &str = "📏 Возраст";
        pub const EDIT_CITY: &str = "🏘 Город";
        pub const EDIT_HEARING_LEVEL: &str = "👂 Уровень слуха";
        pub const EDIT_DESCRIPTION: &str = "📝 Описание";
        pub const EDIT_PHOTO: &str = "🖼 Фото";
    }

    pub trait MakeKeyboard {
        fn keyboard() -> KeyboardMarkup;
    }

    pub trait MakeKeyboardInline {
        fn keyboard_inline() -> InlineKeyboardMarkup;
    }

    pub struct SetHearingLevel;

    impl SetHearingLevel {
        pub fn keyboard(user_sex: Sex) -> InlineKeyboardMarkup {
            match user_sex {
                Sex::Male => InlineKeyboardMarkup::new([
                    [InlineKeyboardButton::callback(DEAF_BOY, CallbackData::SHR { hearing_level: profile::HearingLevel::CompletelyDeaf })],
                    [InlineKeyboardButton::callback(HEARING_IMPAIRED_BOY, CallbackData::SHR { hearing_level: profile::HearingLevel::HearingImpaired })],
                    [InlineKeyboardButton::callback(HEARING_BOY, CallbackData::SHR { hearing_level: profile::HearingLevel::Hearing })],
                ]),
                Sex::Female => InlineKeyboardMarkup::new([
                    [InlineKeyboardButton::callback(DEAF_GIRL, CallbackData::SHR { hearing_level: profile::HearingLevel::CompletelyDeaf })],
                    [InlineKeyboardButton::callback(HEARING_IMPAIRED_GIRL, CallbackData::SHR { hearing_level: profile::HearingLevel::HearingImpaired })],
                    [InlineKeyboardButton::callback(HEARING_GIRL, CallbackData::SHR { hearing_level: profile::HearingLevel::Hearing })],
                ])
            }
        }
    }

    pub struct EditProfile;

    impl EditProfile {
        pub fn keyboard() -> InlineKeyboardMarkup {
            InlineKeyboardMarkup::new([
                [InlineKeyboardButton::callback(EDIT_NAME, CallbackData::EPD { profile_field: ProfileField::Name })],
                [InlineKeyboardButton::callback(EDIT_AGE, CallbackData::EPD { profile_field: ProfileField::Age })],
                [InlineKeyboardButton::callback(EDIT_CITY, CallbackData::EPD { profile_field: ProfileField::City })],
                [InlineKeyboardButton::callback(EDIT_HEARING_LEVEL, CallbackData::EPD { profile_field: ProfileField::HearingLevel })],
                [InlineKeyboardButton::callback(EDIT_DESCRIPTION, CallbackData::EPD { profile_field: ProfileField::Description })],
                [InlineKeyboardButton::callback(EDIT_PHOTO, CallbackData::EPD { profile_field: ProfileField::Photo })],
                [InlineKeyboardButton::callback(FINISH, CallbackData::FED)],
            ])
        }
    }

    pub struct LookingAtProfiles;

    impl MakeKeyboard for LookingAtProfiles {
        fn keyboard() -> KeyboardMarkup {
            KeyboardMarkup::new([[
                KeyboardButton::new(LIKE),
                KeyboardButton::new(DISLIKE),
                KeyboardButton::new(MENU),
            ]])
            .resize_keyboard(true)
        }
    }

    pub struct EnterProfileEditingMode;

    impl EnterProfileEditingMode {
        pub fn keyboard() -> InlineKeyboardMarkup {
            InlineKeyboardMarkup::new([
                [InlineKeyboardButton::callback(EDIT, CallbackData::EPEM)]
            ])
        }
    }

    pub struct RequestLocation;

    impl MakeKeyboard for RequestLocation {
        fn keyboard() -> KeyboardMarkup {
            KeyboardMarkup::new([[KeyboardButton::new(LOCATION).request(ButtonRequest::Location)]])
                .resize_keyboard(true)
                .one_time_keyboard(true)
        }
    }

    pub struct SelectSex;

    impl MakeKeyboard for SelectSex {
        fn keyboard() -> KeyboardMarkup {
            KeyboardMarkup::new([[KeyboardButton::new(BOY), KeyboardButton::new(GIRL)]])
                .resize_keyboard(true)
                .one_time_keyboard(true)
        }
    }

    pub struct SelectWantToMeet;

    impl MakeKeyboard for SelectWantToMeet {
        fn keyboard() -> KeyboardMarkup {
            KeyboardMarkup::new([
                vec![
                    KeyboardButton::new(WANT_A_BOY),
                    KeyboardButton::new(WANT_A_GIRL),
                ],
                vec![KeyboardButton::new(WHATEVER)],
            ])
            .resize_keyboard(true)
            .one_time_keyboard(true)
        }
    }

    pub struct HearingLevel;

    impl HearingLevel {
        pub fn keyboard(user_sex: Sex) -> KeyboardMarkup {
            let markup = match user_sex {
                Sex::Male => KeyboardMarkup::new([
                    [KeyboardButton::new(DEAF_BOY)],
                    [KeyboardButton::new(HEARING_IMPAIRED_BOY)],
                    [KeyboardButton::new(HEARING_BOY)],
                ]),
                Sex::Female => KeyboardMarkup::new([
                    [KeyboardButton::new(DEAF_GIRL)],
                    [KeyboardButton::new(HEARING_IMPAIRED_GIRL)],
                    [KeyboardButton::new(HEARING_GIRL)],
                ]),
            };
            markup.resize_keyboard(true).one_time_keyboard(true)
        }
    }

    pub struct LeaveEmptyDescription;

    impl MakeKeyboardInline for LeaveEmptyDescription {
        fn keyboard_inline() -> InlineKeyboardMarkup {
            InlineKeyboardMarkup::new([[InlineKeyboardButton::callback(LEAVE_EMPTY, CallbackData::LED)]])
        }
    }

    impl MakeKeyboard for LeaveEmptyDescription {
        fn keyboard() -> KeyboardMarkup {
            KeyboardMarkup::new([[KeyboardButton::new(LEAVE_EMPTY)]])
                .one_time_keyboard(true)
                .resize_keyboard(true)
        }
    }

    pub struct Menu;

    impl MakeKeyboard for Menu {
        fn keyboard() -> KeyboardMarkup {
            KeyboardMarkup::new([[
                KeyboardButton::new(WATCH_PROFILES),
                KeyboardButton::new(MY_PROFILE),
                KeyboardButton::new(WHO_LIKES_ME),
            ]])
            .resize_keyboard(true)
        }
    }
}
