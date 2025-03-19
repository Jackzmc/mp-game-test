mod game;
mod network;
mod main_menu;
mod def;

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
use ::rand::SeedableRng;
use ::rand::prelude::*;

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
        std::process::exit(0);
    }

    while !game.is_authenticated() {
        if let Some(event) = game.net_mut().next_event() {
            debug!("[main->login] got event, processing: {:?}", event);
            game.process_event(event);
        }
    }
    // let mut cam = Camera2D::from_display_rect(Rect::new(0.0, 0.0, screen_width(), screen_height()));
    game.cam.camera.position = vec3(20.0, 20.0, 5.0);
    game.cam.camera.target = vec3(0.0, 0.0, 0.0);
    debug!("starting game loop");
    let mut instant: Option<Instant> = None;
    let mut frame: u64 = 0;
    let mut last_mouse_pos = vec2(0.0, 0.0);
    // While connected:?
    while !is_quit_requested() {
        let prev_frame_time = instant.map(|instant| instant.elapsed()); //instant.elapsed();
        instant = Some(Instant::now());

        // Check if there's any event to process
        if let Some(frame_delta) = prev_frame_time {
            game.update();
        }
        game.render();
        if frame % 10 == 0 {
            if let Some(prev_frame_time) = prev_frame_time {
                let fps = 1.0 / prev_frame_time.as_secs_f64();
                game.fps_calc.add_fps(fps);
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

fn draw_cube_rot(pos: Vec3, rot: Vec3, color: Color) {
    let u = rot.normalize();
    let quat = Quat::from_axis_angle(u, 50.0f32.to_radians());
}




fn get_direction_vector(direction: &Vec3, ang: &Vec3) -> Vec3 {
    Vec3 {
        x: direction.x * ang.z.cos() - direction.z * ang.x.cos() * ang.z.sin() - direction.y * ang.x.sin() * ang.z.sin(),
        y: direction.x * ang.z.sin() + direction.z * ang.x.cos() * ang.z.cos() + direction.y * ang.x.sin() * ang.z.cos(),
        z: direction.z * ang.x.sin() - direction.y * ang.x.cos(),
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