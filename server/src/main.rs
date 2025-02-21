mod game;
mod network;

use std::io::{stdin, stdout};
use std::time::{Duration, Instant};
use clap::Parser;
use log::{debug, error, info, trace};
use tokio::net::UdpSocket;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use mp_game_test_common::game::{CommonGameInstance, PlayerData};
use mp_game_test_common::{setup_logger, PacketSerialize};
use rand::random;
use tokio::time::{interval, MissedTickBehavior};
use mp_game_test_common::events_client::ClientEvent;
use mp_game_test_common::events_server::ServerEvent;
use mp_game_test_common::packet::Packet;
use crate::game::{GameInstance, PacketResponse};

const TICK_RATE: u8 = 30;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long, alias = "tickrate", default_value_t = 30)]
    tick_rate: u8,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opt = Args::parse();
    setup_logger();

    let term = console::Term::stdout();

    let mut game = GameInstance::new(opt.tick_rate);
    let mut input = String::new();
    loop {
        if let Ok(n) = stdin().read_line(&mut input) {
            term.clear_last_lines(2);
            input = input.trim_end_matches('\n').to_string();
            println!("> {}", input);
            if let Err(e) = game.exec_server_cmd(&mut input) {
                error!(" {}", e);
            }
        }
        game.tick().await;
    }
}
