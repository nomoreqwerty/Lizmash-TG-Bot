use crate::{common, profile};
use crate::common::{conversation};
use crate::common::keyboard::{EditProfile, EnterProfileEditingMode, LookingAtProfiles, MakeKeyboard, Menu};
use crate::common::structs::SearchData;
use crate::database::Database;
use crate::defines::{BotDialogue, TeloxideResult};
use crate::profile::{Like, View};
use crate::state::State;
use crate::user::MyUserId;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{MessageId, ParseMode};
use crate::callback::ProfileField;

pub async fn start_looking_at_profiles(
    bot: Bot,
    db: Arc<Database>,
    dialogue: BotDialogue,
    user_id: impl Into<MyUserId>,
) -> TeloxideResult {
    let user_id = user_id.into();

    let suggestion = match common::next_suggestion(db.clone(), user_id).await {
        Some(profile) => profile,
        None => {
            conversation::send_no_suggestion(bot, user_id).await?;
            return Ok(());
        }
    };

    bot.send_message(user_id, "Смотрим анкеты")
        .reply_markup(LookingAtProfiles::keyboard())
        .await?;

    dialogue
        .update(State::LookingAtProfiles {
            data: SearchData {
                user_profile: db.get_profile(user_id).await.unwrap(),
                profile_id: *suggestion.id(),
            },
        })
        .await?;

    conversation::send_profile(bot, user_id, &suggestion).await?;

    Ok(())
}

pub async fn start_looking_at_likes(
    bot: Bot,
    db: Arc<Database>,
    dialogue: BotDialogue,
    user_id: impl Into<MyUserId>,
) -> TeloxideResult {
    let user_id = user_id.into();

    let profile_who_have_liked = match common::next_profile_who_have_liked(db.clone(), user_id).await {
        Some(profile) => profile,
        None => {
            bot.send_message(user_id, "🫥 Никто пока не лайкнул твою анкету")
                .await?;
            return Ok(());
        }
    };

    bot.send_message(user_id, "Смотрим, кто тебя лайкнул")
        .reply_markup(LookingAtProfiles::keyboard())
        .await?;

    dialogue
        .update(State::LookingAtProfilesWhoHaveLiked {
            data: SearchData {
                user_profile: db.get_profile(user_id).await.unwrap(),
                profile_id: *profile_who_have_liked.id(),
            },
        })
        .await?;

    conversation::send_profile(bot, user_id, &profile_who_have_liked).await?;

    Ok(())
}

pub async fn match_likes(
    bot: Bot,
    db: Arc<Database>,
    dialogue: BotDialogue,
    user_id: impl Into<MyUserId>,
    data: &SearchData,
) -> TeloxideResult {
    let user_id = user_id.into();

    if db.get_like_to_user(user_id).await.is_none() {
        bot.send_message(user_id, "Поздно, срок действия лайка уже истёк")
            .reply_markup(Menu::keyboard())
            .await?;
        dialogue.reset().await?;
        return Ok(());
    }

    let first_liked_user = db.get_user(data.profile_id).await.unwrap();
    let dialogue_user = db.get_user(user_id).await.unwrap();

    bot.clone()
        .parse_mode(ParseMode::Html)
        .send_message(
            user_id,
            format!(
                "Удачного знакомства - <a href=\"t.me/{}\">жми на меня</a>",
                first_liked_user.username()
            ),
        )
        .disable_web_page_preview(true)
        .await?;

    bot.clone()
        .parse_mode(ParseMode::Html)
        .send_message(data.profile_id, "У вас взаимный лайк 👇")
        .await?;

    conversation::send_profile(bot.clone(), data.profile_id, &data.user_profile).await?;

    bot.parse_mode(ParseMode::Html)
        .send_message(
            data.profile_id,
            format!(
                "🥳 Удачного знакомства - <a href=\"t.me/{}\">жми на меня</a>",
                dialogue_user.username()
            ),
        )
        .disable_web_page_preview(true)
        .await?;

    Ok(())
}

#[inline]
pub async fn like_profile(
    bot: Bot,
    db: Arc<Database>,
    user_id: impl Into<MyUserId>,
    data: &SearchData,
) -> TeloxideResult {
    let user_id = user_id.into();

    db.add_like(Like::new(user_id, data.profile_id, None)).await;
    db.add_view(View::new(user_id, data.profile_id, true)).await;

    let _ = bot
        .parse_mode(ParseMode::Html)
        .send_message(data.profile_id, "✨ Тебя кто-то лайкнул. Посмотреть можно в разделе <b>лайки</b> в меню")
        .await;

    Ok(())
}

#[inline]
pub async fn dislike_profile(
    db: Arc<Database>,
    user_id: impl Into<MyUserId>,
    data: &SearchData,
) -> TeloxideResult {
    db.add_view(View::new(user_id.into(), data.profile_id, false))
        .await;
    Ok(())
}

pub async fn send_new_suggestion(
    bot: Bot,
    db: Arc<Database>,
    dialogue: BotDialogue,
    user_id: impl Into<MyUserId>,
    mut data: SearchData,
) -> TeloxideResult {
    let user_id = user_id.into();

    match common::next_suggestion(db, user_id).await {
        Some(new_suggestion) => {
            data.profile_id = *new_suggestion.id();

            dialogue.update(State::LookingAtProfiles { data }).await?;

            conversation::send_profile(bot, user_id, &new_suggestion).await?;
        }
        None => {
            conversation::send_no_suggestion(bot, user_id).await?;

            dialogue.reset().await?;
        }
    }

    Ok(())
}

pub async fn give_new_liked_profile(
    bot: Bot,
    db: Arc<Database>,
    dialogue: BotDialogue,
    user_id: impl Into<MyUserId>,
    mut data: SearchData,
) -> TeloxideResult {
    let user_id = user_id.into();

    match common::next_profile_who_have_liked(db.clone(), user_id).await {
        Some(liked_profile) => {
            data.profile_id = *liked_profile.id();

            dialogue
                .update(State::LookingAtProfilesWhoHaveLiked { data })
                .await?;

            conversation::send_profile(bot, user_id, &liked_profile).await?;
        }
        None => {
            conversation::send_likes_are_over_now_search(bot.clone(), user_id).await?;

            send_new_suggestion(bot, db.clone(), dialogue, user_id, data).await?;
        }
    }

    Ok(())
}

pub async fn set_hearing_level(
    bot: Bot,
    db: Arc<Database>,
    q: CallbackQuery,
    hearing_level: profile::HearingLevel
) -> TeloxideResult {
    db.set_profile_hearing_level(q.from.id, hearing_level).await;
    bot.edit_message_text(q.from.id, q.message.as_ref().unwrap().id, common::text::PROFILE_EDIT_MODE)
        .await?;
    bot.edit_message_reply_markup(q.from.id, q.message.as_ref().unwrap().id)
        .reply_markup(EditProfile::keyboard())
        .await?;
    Ok(())
}

pub async fn leave_empty_description(
    bot: Bot,
    db: Arc<Database>,
    q: CallbackQuery,
) -> TeloxideResult {
    db.set_profile_description(q.from.id, None).await;
    bot.edit_message_text(q.from.id, q.message.as_ref().unwrap().id, common::text::PROFILE_EDIT_MODE)
        .await?;
    bot.edit_message_reply_markup(q.from.id, q.message.as_ref().unwrap().id)
        .reply_markup(EditProfile::keyboard())
        .await?;
    Ok(())
}

pub async fn finish_editing_profile(bot: Bot, db: Arc<Database>, dialogue: BotDialogue, q: CallbackQuery) -> TeloxideResult {
    bot.delete_message(q.from.id, q.message.as_ref().unwrap().id).await?;
    bot.delete_message(q.from.id, MessageId(q.message.as_ref().unwrap().id.0 - 1)).await?;

    bot.send_message(q.from.id, "✨ Твоя новая анкета").await?;

    let profile = db.get_profile(q.from.id).await.unwrap();

    conversation::send_profile_with_keyboard_inline(bot, q.from.id, &profile, EnterProfileEditingMode::keyboard()).await?;

    dialogue.reset().await?;

    Ok(())
}

#[inline]
pub async fn enter_menu(
    bot: Bot,
    dialogue: BotDialogue,
    user_id: impl Into<MyUserId>,
) -> TeloxideResult {
    dialogue.update(State::None).await?;
    conversation::send_menu(bot, user_id.into()).await?;
    Ok(())
}

pub async fn enter_profile_editing_mode(
    bot: Bot,
    q: CallbackQuery,
) -> TeloxideResult {
    bot.edit_message_reply_markup(q.from.id, q.message.as_ref().unwrap().id)
        .await?;

    let message: Message = bot.send_message(q.from.id, common::text::PROFILE_EDIT_MODE)
        .await?;

    bot.edit_message_reply_markup(message.chat.id, message.id)
        .reply_markup(EditProfile::keyboard())
        .await?;

    Ok(())
}

pub async fn set_profile_editing_handler(
    bot: Bot,
    db: Arc<Database>,
    q: CallbackQuery,
    dialogue: BotDialogue,
    profile_field: ProfileField
) -> TeloxideResult {
    match profile_field {
        ProfileField::Name => profile_edit_handler_setters::name(bot, q, dialogue, profile_field).await?,
        ProfileField::Age => profile_edit_handler_setters::age(bot, q, dialogue, profile_field).await?,
        ProfileField::City => profile_edit_handler_setters::city(bot, q, dialogue, profile_field).await?,
        ProfileField::HearingLevel => profile_edit_handler_setters::hearing_level(bot, db, q).await?,
        ProfileField::Description => profile_edit_handler_setters::description(bot, q, dialogue, profile_field).await?,
        ProfileField::Photo => profile_edit_handler_setters::photo(bot, q, dialogue, profile_field).await?,
    }

    Ok(())
}

pub(crate) mod profile_edit_handler_setters {
    use crate::common::keyboard::{LeaveEmptyDescription, MakeKeyboardInline, SetHearingLevel};
    use super::*;

    #[inline]
    pub(crate) async fn name(bot: Bot, q: CallbackQuery, dialogue: BotDialogue, profile_field: ProfileField) -> TeloxideResult {
        bot.edit_message_text(q.from.id, q.message.as_ref().unwrap().id, "✒ Отправь своё имя").await?;
        dialogue.update(State::EditingProfile { profile_field, callback_query: q }).await?;
        Ok(())
    }

    #[inline]
    pub(crate) async fn age(bot: Bot, q: CallbackQuery, dialogue: BotDialogue, profile_field: ProfileField) -> TeloxideResult {
        bot.edit_message_text(q.from.id, q.message.as_ref().unwrap().id, "📏 Отправь свой возраст").await?;
        dialogue.update(State::EditingProfile { profile_field, callback_query: q }).await?;
        Ok(())
    }

    #[inline]
    pub(crate) async fn city(bot: Bot, q: CallbackQuery, dialogue: BotDialogue, profile_field: ProfileField) -> TeloxideResult {
        bot.edit_message_text(q.from.id, q.message.as_ref().unwrap().id, "🏘 Отправь свой город").await?;
        dialogue.update(State::EditingProfile { profile_field, callback_query: q }).await?;
        Ok(())
    }

    #[inline]
    pub(crate) async fn hearing_level(bot: Bot, db: Arc<Database>, q: CallbackQuery) -> TeloxideResult {
        bot.edit_message_text(q.from.id, q.message.as_ref().unwrap().id, "👂 Выбери свой уровень слуха").await?;
        bot.edit_message_reply_markup(q.from.id, q.message.as_ref().unwrap().id)
            .reply_markup(SetHearingLevel::keyboard(*db.get_profile(q.from.id).await.unwrap().sex()))
            .await?;
        Ok(())
    }

    #[inline]
    pub(crate) async fn description(bot: Bot, q: CallbackQuery, dialogue: BotDialogue, profile_field: ProfileField) -> TeloxideResult {
        bot.edit_message_text(q.from.id, q.message.as_ref().unwrap().id, "📝 Придумай себе новое описание").await?;
        bot.edit_message_reply_markup(q.from.id, q.message.as_ref().unwrap().id)
            .reply_markup(LeaveEmptyDescription::keyboard_inline())
            .await?;
        dialogue.update(State::EditingProfile { profile_field, callback_query: q }).await?;
        Ok(())
    }

    #[inline]
    pub(crate) async fn photo(bot: Bot, q: CallbackQuery, dialogue: BotDialogue, profile_field: ProfileField) -> TeloxideResult {
        bot.edit_message_text(q.from.id, q.message.as_ref().unwrap().id, "🖼 Отправь своё новое фото").await?;
        dialogue.update(State::EditingProfile { profile_field, callback_query: q }).await?;
        Ok(())
    }
}
