mod game;
mod network;

use crate::game::GameInstance;
use crate::network::NetClient;
use macroquad::audio::play_sound;
use macroquad::prelude::*;
use mp_game_test_common::def::{Position, MAX_PLAYERS};
use mp_game_test_common::events_client::ClientEvent;
use mp_game_test_common::game::Action;
use mp_game_test_common::setup_logger;
use std::net::SocketAddr;
use std::sync::mpsc::channel;
use std::thread;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

fn window_conf() -> Conf {
    Conf {
        window_title: "Multiplayer Test".to_owned(),
        fullscreen: false,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let args = std::env::args();
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
    let mut cam = Camera2D {
        ..Default::default()
    };
    debug!("starting draw loop");
    loop {
        // Check if there's any event to process
        if let Some(event) = game.net.next_event() {
            debug!("got event, processing: {:?}", event);
            game.process_event(event);
        }
        clear_background(WHITE);

        set_camera(&mut cam);
        draw_line(-0.4, 0.4, -0.8, 0.9, 0.05, BLUE);
        draw_rectangle(-0.3, 0.3, 0.2, 0.2, GREEN);
        draw_rectangle(0.0, 0.0, 200.0, 200.0, GREEN);

        // TODO: draw players
        for i in 0..MAX_PLAYERS {
            if let Some(player) = &game.game.players[i] {
                let pos = cam.screen_to_world(Vec2::new(player.position.x, player.position.y));
                draw_rectangle(pos.x, pos.y, 0.1, 0.1, BLACK);
                draw_text(
                    &player.name,
                    pos.x,
                    pos.y - 2.0,
                    20.0,
                    RED,
                );
            }
        }

        // TODO: get player pos mut
        set_action(&mut game, Action::Forward, KeyCode::W);
        set_action(&mut game, Action::Backward, KeyCode::S);
        set_action(&mut game, Action::Left, KeyCode::A);
        set_action(&mut game, Action::Right, KeyCode::D);
        if let Some(player) = game.player_mut() {
            // cam.offset = Vec2::new(player.position.x, player.position.y);
            // trace!("cam.offset={}", cam.offset);
        }

        set_default_camera();
        if let Some(client_id) = game.client_id() {
            draw_text(
                &format!("Id: {}", client_id),
                screen_width() - 200.0,
                20.0,
                20.0,
                DARKGRAY,
            );
        } else {
            draw_text(
                "Authenticating...",
                screen_width() - 200.0,
                20.0,
                20.0,
                DARKGRAY,
            );
        }
        draw_text(
            &game.net.event_queue_len().to_string(),
            20.0,
            20.0,
            20.0,
            DARKGRAY,
        );

        next_frame().await
    }
}

fn set_action(game: &mut GameInstance, action: Action, key_code: KeyCode) {
    if is_key_pressed(key_code) {
        game.set_action(action, true).ok();
    } else if game.has_action(action) && is_key_released(key_code) {
        game.set_action(action, false).ok();
    }
}
