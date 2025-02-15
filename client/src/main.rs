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
use mp_game_test_common::game::Action;
use mp_game_test_common::setup_logger;
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
    let args =  std::env::args();
    setup_logger();

    let server_ip = args.skip(1).next().expect("no ip specified");

    let addr: SocketAddr = server_ip.parse().expect("bad ip");
    let mut game = GameInstance::new(addr);
    game.login("Test User".to_string()).unwrap();

    let mut pos = Position::new(0.0, 0.0, 0.0);
    while !game.is_authenticated() {
        if let Some(event) = game.net.next_event() {
            debug!("got event, processing: {:?}", event);
            game.process_event(event);
        }
    }

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
        for i in 0..MAX_PLAYERS {if let Some(player) = &game.game.players[i] {
                draw_rectangle(player.position.x, player.position.y, 80.0, 80.0, BLACK);
            }}
        for i in 0..MAX_PLAYERS {
            if let Some(player) = &game.game.players[i] {
                draw_text(&player.name, player.position.x, player.position.y - 2.0, 12.0, RED);
            }
        }

        // TODO: get player pos mut
        if let Some(player) = game.player_mut() {
            if is_key_down(KeyCode::W) {
                game.perform_action(Action::Forward);
            }
            if is_key_down(KeyCode::S) {
                game.perform_action(Action::Backward);
            }
            if is_key_down(KeyCode::A) {
                game.perform_action(Action::Left);
            }
            if is_key_down(KeyCode::D) {
                game.perform_action(Action::Right);
            }
        }

        next_frame().await
    }
}
