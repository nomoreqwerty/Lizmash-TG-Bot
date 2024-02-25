#![feature(async_closure)]

mod application;
mod callback;
mod commands;
mod common;
mod database;
mod defines;
mod error;
mod maps;
mod perform;
mod profile;
mod state;
mod user;

#[tokio::main]
async fn main() {
    application::DeafBot::main().await
}

