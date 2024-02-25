use crate::commands::Command;
use crate::common::conversation;
use crate::database::Database;
use crate::maps::Maps;
use crate::state::State;
use crate::*;
use std::error::Error;
use std::sync::Arc;
use teloxide::dispatching::dialogue::InMemStorage;
use teloxide::dispatching::{dialogue, UpdateHandler};
use teloxide::dptree::deps;
use teloxide::prelude::*;

pub struct DeafBot;

impl DeafBot {
    pub async fn main() {
        pretty_env_logger::init_custom_env("DEAFBOT_LOG");

        log::info!("initializing configuration");
        let config = Configuration::init();

        log::info!("initializing database");
        let database = Database::init().await;
        let maps = Maps::init(config.yandex_maps_api_key.clone()).await;

        Self::run(config, database, maps).await;
    }

    async fn run(config: Arc<Configuration>, database: Arc<Database>, maps: Arc<Maps>) {
        let schema = schema();
        let bot = Bot::new(&config.bot_token);

        let state_storage = InMemStorage::<State>::new();

        log::info!("dispatching the bot");

        Dispatcher::builder(bot, schema)
            .dependencies(deps![state_storage, database, maps])
            .enable_ctrlc_handler()
            .build()
            .dispatch()
            .await;
    }
}

pub struct Configuration {
    bot_token: String,
    yandex_maps_api_key: String,
}

impl Configuration {
    pub fn init() -> Arc<Self> {
        let config_string = Configuration::read_configuration_file();
        Arc::new(Configuration::parse_configuration_string(&config_string))
    }

    pub fn parse_configuration_string(string: &str) -> Self {
        let json: serde_json::Value = serde_json::from_str(string)
            .expect("Unable to parse `config.json` into valid json");

        Self {
            bot_token: json["bot_token"].as_str()
                .expect("Unable to parse `token` value in `config.json`")
                .to_owned(),
            yandex_maps_api_key: json["yandex_maps_api_key"].as_str()
                .expect("Unable to parse `yandex_maps_api_key` value in `config.json`")
                .to_owned(),
        }
    }

    pub fn read_configuration_file() -> String {
        std::fs::read_to_string("config.json")
            .expect("Unable to read `config.json`")
    }
}

pub fn schema() -> UpdateHandler<Box<dyn Error + Send + Sync>> {
    dialogue::enter::<Update, InMemStorage<State>, State, _>()
        .branch(
            Update::filter_message()
                .branch(
                    dptree::filter(|msg: Message| !(msg.chat.is_private() && msg.chat.is_chat()))
                        .endpoint(conversation::bot_works_only_in_chats)
                )
                .branch(
                    dptree::filter(|msg: Message| {
                        msg.from().map_or(false, |user| user.username.is_none())
                    })
                    .endpoint(conversation::send_username_is_needed),
                )
                .branch(
                    dptree::filter_async(async move |msg: Message, db: Arc<Database>| {
                        db.get_profile(msg.chat.id).await.is_none()
                    })
                    .branch(
                        teloxide::filter_command::<Command, _>()
                            .endpoint(commands::handle_user_without_profile),
                    )
                    .branch(
                        dptree::case![State::CreatingProfile {
                            profile_builder,
                            state
                        }]
                        .endpoint(state::build_profile),
                    ),
                )
                .branch(
                    dptree::entry()
                        .branch(teloxide::filter_command::<Command, _>().endpoint(commands::handle_command))
                        .branch(dptree::case![State::None].endpoint(state::handle_message))
                        .branch(
                            dptree::case![State::LookingAtProfiles { data }]
                                .endpoint(state::act_on_profile),
                        )
                        .branch(
                            dptree::case![State::LookingAtProfilesWhoHaveLiked { data }]
                                .endpoint(state::look_at_likes),
                        )
                        .branch(
                            dptree::case![State::EditingProfile { profile_field, callback_query }]
                                .endpoint(state::edit_profile)
                        )
                ),
        )
        .branch(Update::filter_callback_query().endpoint(callback::handle))
}
