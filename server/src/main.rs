mod game;
mod network;
mod cmds;

use std::io::{stdin, stdout, Read};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{channel, Receiver};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use clap::Parser;
use log::{debug, error, info, trace};
use tokio::net::UdpSocket;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use mp_game_test_common::game::{CommonGameInstance, PlayerData};
use mp_game_test_common::{setup_logger, PacketSerialize};
use rand::random;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt};
use tokio::time::{interval, MissedTickBehavior};
use mp_game_test_common::events_client::ClientEvent;
use mp_game_test_common::events_server::ServerEvent;
use mp_game_test_common::packet::Packet;
use crate::cmds::{register_commands, CommandArgs};
use crate::game::{GameInstance, PacketResponse};

const TICK_RATE: u8 = 30;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long, alias = "tickrate", default_value_t = 30)]
    tick_rate: u8,

    #[arg(long, default_value = "0.0.0.0")]
    ip: String,

    #[arg(long, short = 'p', default_value_t = 3566)]
    port: u16
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opt = Args::parse();
    setup_logger();

    let mut game = GameInstance::new(opt.tick_rate);
    register_commands(&mut game);

    let term = console::Term::stdout();

    { // Handle Ctrl+C
        let pending_shutdown = game.shutdown_requested.clone();
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.unwrap();
            info!("CTRL+C Received, shutting down...");
            pending_shutdown.store(true, std::sync::atomic::Ordering::SeqCst);
            tokio::signal::ctrl_c().await.unwrap();
            info!("Forcing shutdown");
            std::process::exit(0);
        })
    };

    let stdin = setup_stdin_channel();

    // Main game loop
    while !game.is_shutdown_requested() {
        if let Ok(mut input) = stdin.try_recv() {
            term.clear_last_lines(2).ok();
            input = input.trim_end_matches('\n').to_string();
            println!("> {}", input);

            if let Err(e) = game.exec_cmd(&input, None) {
                error!(" {}", e);
            }

            input.clear();
        }
        game.tick().await;
    }
    debug!("shutdown start");
    game._shutdown();
    Ok(())
}

// Sends stdin lines to channel. Needs its own thread to prevent read_line blocking main loop
fn setup_stdin_channel() -> Receiver<String> {
    let (tx, rx) = channel();
    let mut input = String::new();
    // don't care about joining - lifetime of program
    std::thread::spawn(move || {
        loop {
            if let Ok(n) = stdin().read_line(&mut input) {
                tx.send(input).unwrap();
                input = String::new();
            }
        }
    });
    rx
}