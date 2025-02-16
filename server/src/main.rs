mod game;
mod network;

use std::time::{Duration, Instant};
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    let mut game = GameInstance::new(TICK_RATE);
    loop {
        game.tick().await;
    }
}
