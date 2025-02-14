mod network;
mod game;

use std::net::SocketAddr;
use std::sync::mpsc::channel;
use std::thread;
use macroquad::audio::play_sound;
use macroquad::prelude::*;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use mp_game_test_common::def::{Position, MAX_PLAYERS};
use mp_game_test_common::events_client::ClientEvent;
use crate::game::GameInstance;
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
    let mut game = GameInstance::new(addr);
    game.login("Test User".to_string()).unwrap();

    let mut pos = Position::new(0.0, 0.0, 0.0);

    loop {
        // Check if there's any event to process
        if let Some(event) = game.net.next_event() {
            debug!("got event, processing: {:?}", event);
            game.process_event(event);
        }
        clear_background(WHITE);
        if let Some(client_id) = game.client_id() {
            draw_text(&format!("Id: {}",client_id), screen_width() - 200.0, 20.0, 20.0, DARKGRAY);
        } else {
            draw_text("Authenticating...", screen_width() - 200.0, 20.0, 20.0, DARKGRAY);
        }
        draw_text(&game.net.event_queue_len().to_string(), 20.0, 20.0, 20.0, DARKGRAY);

        // TODO: draw players
        for i in 0..MAX_PLAYERS {
            if let Some(player) = &game.game.players[i] {
                draw_rectangle(player.position.x, player.position.y, 80.0, 80.0, BLACK);
            }
        }
        for i in 0..MAX_PLAYERS {
            if let Some(player) = &game.game.players[i] {
                draw_text(&player.name, player.position.x, player.position.y - 2.0, 12.0, RED);
            }
        }

        // TODO: get player pos mut
        if game.is_authenticated() {
            if is_key_pressed(KeyCode::W) {
                pos.y -= 1.0;
                if pos.y < 0.0 {
                    pos.y = 0.0;
                }
                let ev_move = ClientEvent::Move {
                    position: pos
                };
                trace!("{:?}", pos);
                let auth_id = game.auth_id().unwrap();
                game.send(&ev_move).unwrap();
            }
            if is_key_pressed(KeyCode::S) {
                pos.y += 1.0;
                if pos.y > screen_height() {
                    pos.y = screen_height();
                }
                let ev_move = ClientEvent::Move {
                    position: pos
                };
                trace!("{:?}", pos);
                let auth_id = game.auth_id().unwrap();
                game.send(&ev_move).unwrap();
            }
        }

        next_frame().await
    }
}
