mod config;
mod controllers;
mod db;
mod services;

use db::*;
use services::*;

use thrust_macros::init;
init!();

#[tokio::main]
async fn main() {
    run().await;
}
