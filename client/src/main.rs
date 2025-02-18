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
use std::time::Instant;
use macroquad::{hash, ui};
use macroquad::ui::{root_ui, widgets, Id};
use macroquad::ui::widgets::{Editbox, Label};
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
    prevent_quit();
    let args = std::env::args();
    setup_logger();

    let mut ip_addr = "127.0.0.1:3566".to_string();
    let mut server_ip = None;
    let window_style = root_ui()
        .style_builder()
        .background_margin(RectOffset::new(32.0, 76.0, 44.0, 20.0))
        .margin(RectOffset::new(0.0, -40.0, 0.0, 0.0))
        .build();
    let window_size = vec2(542.0, 430.0);
    while server_ip.is_none() && !is_quit_requested() {
        clear_background(WHITE);
        widgets::Window::new(hash!(), vec2(screen_width() / 2.0 - window_size.x / 2.0,
                                           screen_height() / 2.0 - window_size.y / 2.0,), window_size)
            .label("Multiplayer Test")
            .titlebar(false)
            .movable(false)
            .ui(&mut *root_ui(), |ui| {
                ui.label(None, "Direct Connect");
                ui.editbox(hash!(), vec2(500., 30.), &mut ip_addr);
                if ui.button(None, "Connect") {
                    if let Ok(ip_addr) = ip_addr.parse::<SocketAddr>() {
                        server_ip = Some(ip_addr);
                    }
                }
            });

        next_frame().await
    }
    if is_quit_requested() {
        std::process::exit(0);
    }

    let mut game = GameInstance::new();
    game.connect(server_ip.unwrap());
    game.login("Test User".to_string()).unwrap();

    let mut pos = Position::new(0.0, 0.0, 0.0);
    while !game.is_authenticated() {
        if let Some(event) = game.net_mut().next_event() {
            debug!("[main->login] got event, processing: {:?}", event);
            game.process_event(event);
        }
    }
    let mut cam = Camera2D {
        zoom: Vec2::new(1.0, screen_width() / screen_height()),
        target: Vec2::new(-1.0, -1.0),
        ..Default::default()
    };
    debug!("starting draw loop");
    let mut instant: Option<Instant> = None;
    let mut fps_calc = FpsCounter::new();
    let mut frame: u64 = 0;
    // While connected:?
    while !is_quit_requested() {
        let prev_frame_time = instant.map(|instant| instant.elapsed()); //instant.elapsed();
        instant = Some(Instant::now());

        // Check if there's any event to process
        if let Some(event) = game.net_mut().next_event() {
            debug!("[main->loop] got event, processing: {:?}", event);
            game.process_event(event);
        }
        clear_background(WHITE);

        set_camera(&mut cam);
        // draw_grid(20, 1., BLACK, GRAY);
        draw_line(-0.4, 0.4, -0.8, 0.9, 0.05, BLUE);
        draw_rectangle(-0.3, 0.3, 0.2, 0.2, GREEN);
        draw_rectangle(0.0, 0.0, 200.0, 200.0, GREEN);
        draw_text("TEST", -0.2, 0.2, 0.4, RED);

        // TODO: draw players
        for i in 0..MAX_PLAYERS {
            if let Some(player) = &game.game.players[i] {
                let pos = cam.screen_to_world(Vec2::new(player.position.x, player.position.y));
                draw_rectangle(pos.x, pos.y, 0.1, 0.1, BLACK);
                // draw_cube(Vec3::new(pos.x, pos.y, 1.0), Vec3::new(1.0, 1.0, 1.0), None, BLACK);
                draw_text(
                    &i.to_string(),
                    pos.x,
                    pos.y,
                    0.1,
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
            let text = &format!("Connected. Id#{}", client_id);
            let dim = measure_text(text, None, 20, 1.0);
            draw_text(
                text,
                screen_width() - dim.width - 20.0,
                20.0,
                20.0,
                DARKGRAY,
            );
        }
        draw_text(
            &game.net().process_queue_len().to_string(),
            20.0,
            20.0,
            20.0,
            DARKGRAY,
        );
        if frame % 10 == 0 {
            if let Some(prev_frame_time) = prev_frame_time {
                let fps = 1.0 / prev_frame_time.as_secs_f64();
                fps_calc.add_fps(fps);
            }
        }
        draw_text(
            &format!("{:.0}", fps_calc.average()),
            40.0,
            20.0,
            20.0,
            DARKGRAY,
        );
        frame += 1;
        next_frame().await
    }
    game.disconnect("Disconnect");
}

fn set_action(game: &mut GameInstance, action: Action, key_code: KeyCode) {
    if is_key_pressed(key_code) {
        game.set_action(action, true).ok();
    } else if game.has_action(action) && is_key_released(key_code) {
        game.set_action(action, false).ok();
    }
}

const FRAME_SAMPLES: u16= 100;
struct FpsCounter {
    sum: f64,
    index: u16,
    list: [f64; FRAME_SAMPLES as usize]
}
impl FpsCounter {
    pub fn new() -> Self {
        Self {
            sum: 0.0,
            index: 0,
            list: [0.0; FRAME_SAMPLES as usize],
        }
    }

    pub fn add_fps(&mut self, fps: f64) {
        self.sum -= self.list[self.index as usize];
        self.sum += fps;
        self.list[self.index as usize] = fps;
        self.index = (self.index + 1) % FRAME_SAMPLES;
    }
    pub fn average(&mut self) -> f64 {
        self.sum / FRAME_SAMPLES as f64
    }
}