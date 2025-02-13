mod network;

use std::net::SocketAddr;
use std::sync::mpsc::channel;
use std::thread;
use macroquad::prelude::*;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use mp_game_test_common::def::{ClientEvent, Position};
use crate::network::NetClient;

fn window_conf() -> Conf {
    Conf {
        window_title: "Multiplayer Test".to_owned(),
        fullscreen: false,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::filter::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=trace,mp-game-test-common=trace", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let addr: SocketAddr = "127.0.0.1:3566".parse().unwrap();
    let mut net = NetClient::new(addr);
    net.start().unwrap();

    let mut pos = Position::new(0.0, 0.0, 0.0);

    loop {
        // Check if there's any event to process
        if let Some(event) = net.next_event() {
            debug!("got event: {:?}", event);
        }
        clear_background(RED);

        draw_line(40.0, 40.0, 100.0, 200.0, 15.0, BLUE);
        draw_rectangle(screen_width() / 2.0 - 60.0, 100.0, 120.0, 60.0, GREEN);
        draw_circle(screen_width() - 30.0, screen_height() - 30.0, 15.0, YELLOW);
        draw_text("HELLO", 20.0, 20.0, 20.0, DARKGRAY);

        if macroquad::input::is_key_pressed(KeyCode::W) {
            pos.y += 1.0;
            let ev_move = ClientEvent::Move {
                position: pos
            };
            net.send(&ev_move).unwrap();
        }

        next_frame().await
    }
}
