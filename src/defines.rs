use crate::state::State;
use std::error::Error;
use teloxide::dispatching::dialogue::InMemStorage;
use teloxide::prelude::Dialogue;

pub type TeloxideResult = Result<(), Box<dyn Error + Send + Sync>>;
pub type BotDialogue = Dialogue<State, InMemStorage<State>>;