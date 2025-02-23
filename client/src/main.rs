mod game;
mod network;
mod main_menu;

use crate::game::GameInstance;
use crate::network::NetClient;
use macroquad::audio::play_sound;
use macroquad::prelude::*;
use mp_game_test_common::def::{Vector3, MAX_PLAYERS};
use mp_game_test_common::events_client::ClientEvent;
use mp_game_test_common::game::Action;
use mp_game_test_common::setup_logger;
use std::net::SocketAddr;
use std::ops::Sub;
use std::sync::mpsc::channel;
use std::thread;
use std::time::{Duration, Instant};
use macroquad::{hash, ui};
use macroquad::ui::{root_ui, widgets, Id};
use macroquad::ui::widgets::{Editbox, Label};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use clap::Parser;
use crate::main_menu::MainMenu;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long, alias = "connect")]
    connect_to: Option<String>,

    #[arg(long)]
    name: Option<String>
}

struct Player {
    name: String,
}
impl Player {
    pub fn draw(pos: Vec3) {
        let size = 0.4;
        let size_vec = vec3(size, size, size);
        draw_cube(vec3(pos.x, pos.y + size, pos.z), size_vec, None, BLACK);
        draw_cube(vec3(pos.x, pos.y - size, pos.z), size_vec, None, BLACK);
        draw_cube(vec3(pos.x + size, pos.y, pos.z), size_vec, None, BLACK);
        draw_cube(vec3(pos.x - size, pos.y, pos.z), size_vec, None, BLACK);
    }
}

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
    let args = Args::parse();
    setup_logger();

    let mut server_ip = None;;
    if let Some(connect_to) = args.connect_to {
        server_ip = match connect_to.parse::<SocketAddr>() {
            Ok(addr) => Some(addr),
            Err(e) => {
                error!("--connect: given address is invalid ({}): {}", connect_to, e);
                None
            }
        }
    }

    let mut main_menu = MainMenu::new(args.name, server_ip);
    let mut game = GameInstance::new();

    while !is_quit_requested() {
        main_menu.draw().await;
        if !game.is_connected() {
            if let Some((ip_addr, name)) = main_menu.connect_info() {
                debug!("got connect info {} {} - connecting", ip_addr, name);
                main_menu.set_status_msg(Some("Connecting".to_string()));
                if let Err(e) = game.connect(ip_addr, name) {
                    error!("login error: {}", e);
                    main_menu.set_err_msg(Some(e));
                }
            }
        } else if !game.is_authenticated() {
            // While we are connected but not authenticated, wait for the login event to process
            if let Some(err) = game.net().last_err() {
                error!("login error: {}", err);
                main_menu.set_err_msg(Some(err));
                main_menu.set_status_msg(None);
                game.net_mut().clear_last_err();
            }
            if let Some(event) = game.net_mut().next_event() {
                debug!("[main->main_menu] got event, processing: {:?}", event);
                game.process_event(event);
            }
        } else {
            debug!("authenticated. ready to go.");
            // Authenticated - we are ready
            break;
        }
        next_frame().await;
    }
    if is_quit_requested() {
        return std::process::exit(0);
    }
    let server_ip = main_menu.ip_addr().unwrap();
    let name = main_menu.name().to_owned();

    let mut pos = Vector3::new(0.0, 0.0, 0.0);
    while !game.is_authenticated() {
        if let Some(event) = game.net_mut().next_event() {
            debug!("[main->login] got event, processing: {:?}", event);
            game.process_event(event);
        }
    }
    // let mut cam = Camera2D::from_display_rect(Rect::new(0.0, 0.0, screen_width(), screen_height()));
    game.cam.camera.position = vec3(20.0, 20.0, 20.0);
    debug!("starting draw loop");
    let mut instant: Option<Instant> = None;
    let mut fps_calc = FpsCounter::new();
    let mut frame: u64 = 0;
    let mut last_mouse_pos = vec2(0.0, 0.0);
    // While connected:?
    fn draw(time_delta: &Duration, game: &mut GameInstance, fps_calc: &mut FpsCounter, last_mouse_pos: &mut Vec2) {
        clear_background(WHITE);

        set_camera(&mut game.cam.camera);
        // draw_grid(20, 1., BLACK, GRAY);
        draw_rectangle(0.0, 0.0, 50.0, 50.0, GREEN);
        draw_rectangle(screen_width() - 50.0, 0.0, 50.0, 50.0, GREEN);
        draw_rectangle(screen_width() - 50.0, screen_height() - 50.0, 50.0, 50.0, GREEN);
        draw_rectangle(0.0, screen_height() - 50.0, 50.0, 50.0, GREEN);

        draw_plane(vec3(0.0, 0.0, 0.0), vec2(100.0, 100.0), None, LIGHTGRAY);
        draw_grid(3, 10.0, BLACK, WHITE);

        // TODO: draw players
        for i in 0..MAX_PLAYERS {
            if let Some(player) = &game.game.players[i] {
                Player::draw(Vec3::new(player.position.x, player.position.y, 1.0));
                // draw_rectangle(pos.x, pos.y, 20.0, 20.0, BLACK);
                // draw_cube(vec3(player.position.x, player.position.y, 1.0), Vec3::new(1.0, 1.0, 1.0), None, BLACK);
                // let pos = cam.screen_to_world(Vec2::new(player.position.x, player.position.y));
                let end = vec3(player.position.x + 0.0, player.position.y + 5.0, 1.0);
                draw_line_3d(Vec3::new(player.position.x, player.position.y, 1.0), end, ORANGE);
                draw_text(
                    &i.to_string(),
                    player.position.x,
                    player.position.y,
                    20.0,
                    RED,
                );
            }
        }

        // TODO: get player pos mut
        set_action(game, Action::Forward, KeyCode::W);
        set_action(game, Action::Backward, KeyCode::S);
        set_action(game, Action::Left, KeyCode::A);
        set_action(game, Action::Right, KeyCode::D);
        if let Some(player) = game.player_mut() {
            // cam.offset = Vec2::new(player.position.x, player.position.y);
            // trace!("cam.offset={}", cam.offset);

            if is_mouse_button_down(MouseButton::Left) {
                let mouse_pos = mouse_position_local();
                let mouse_delta = mouse_pos - *last_mouse_pos;
                *last_mouse_pos = mouse_pos;
                let time_delta = time_delta.as_secs_f32();

                // game.cam.rotation.x += mouse_delta.x * time_delta * 1.0;
                // game.cam.rotation.y += mouse_delta.y * time_delta * -1.0;
                // game.cam.rotation.y = clamp(game.cam.rotation.y, -1.5, 1.5);

                set_cursor_grab(true);
                let pos = vec3(player.position.x, player.position.y, player.position.z);
                game.cam.set_target(pos);
            } else {
                set_cursor_grab(false);
            }
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
            let activity_time = game.net().stat().activity_time();
            let (tx, rx) = (activity_time.tx.map(|t| t.elapsed().as_millis().to_string()).unwrap_or("-".to_string()),
                            activity_time.rx.map(|t| t.elapsed().as_millis().to_string()).unwrap_or("-".to_string()));
            draw_text(
                &format!("{} {}", tx, rx),
                screen_width() - dim.width - 20.0,
                50.0,
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
        draw_text(
            &format!("{:.0}", fps_calc.average()),
            40.0,
            20.0,
            20.0,
            DARKGRAY,
        );
    }
    while !is_quit_requested() {
        let prev_frame_time = instant.map(|instant| instant.elapsed()); //instant.elapsed();
        instant = Some(Instant::now());

        // Check if there's any event to process
        if let Some(event) = game.net_mut().next_event() {
            debug!("[main->loop] got event, processing: {:?}", event);
            game.process_event(event);
        }
        if let Some(frame_delta) = prev_frame_time {
            draw(&frame_delta, &mut game, &mut fps_calc, &mut last_mouse_pos);
        }
        if frame % 10 == 0 {
            if let Some(prev_frame_time) = prev_frame_time {
                let fps = 1.0 / prev_frame_time.as_secs_f64();
                fps_calc.add_fps(fps);
            }
        }
        frame += 1;
        next_frame().await
    }
    game.disconnect("Disconnect");
}

enum ActionResult {
    Activated,
    Deactivated,
    None
}

fn set_action(game: &mut GameInstance, action: Action, key_code: KeyCode) -> ActionResult {
    if is_key_pressed(key_code) {
        game.set_action(action, true).ok();
        ActionResult::Activated
    } else if game.has_action(action) && is_key_released(key_code) {
        game.set_action(action, false).ok();
        ActionResult::Deactivated
    } else {
        ActionResult::None
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