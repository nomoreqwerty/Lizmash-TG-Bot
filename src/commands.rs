use crate::common::conversation;
use crate::defines::{BotDialogue, TeloxideResult};
use crate::profile::Profile;
use crate::state::State;
use teloxide::macros::BotCommands;
use teloxide::prelude::*;

#[derive(Debug, Clone, BotCommands)]
#[command(rename_rule = "lowercase", parse_with = "split")]
pub enum Command {
    Start,
}

pub async fn handle_user_without_profile(
    bot: Bot,
    dialogue: BotDialogue,
    msg: Message,
    command: Command,
) -> TeloxideResult {
    match command {
        Command::Start => {
            conversation::default_start(bot, msg.chat.id).await?;

            dialogue
                .update(State::CreatingProfile {
                    profile_builder: Profile::builder(msg.chat.id),
                    state: Default::default(),
                })
                .await?;
        }
    }
    Ok(())
}

pub async fn handle_command(bot: Bot, msg: Message, command: Command) -> TeloxideResult {
    match command {
        Command::Start => conversation::send_menu(bot, msg.chat.id).await?,
    }
    Ok(())
}
