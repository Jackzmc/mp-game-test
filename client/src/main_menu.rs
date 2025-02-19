use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use macroquad::color::WHITE;
use macroquad::hash;
use macroquad::input::is_quit_requested;
use macroquad::math::{vec2, Vec2};
use macroquad::prelude::{clear_background, next_frame, screen_height, screen_width};
use macroquad::ui::{root_ui, widgets};
use rand::{random, random_range};

pub struct MainMenu {
    ip_addr: Option<SocketAddr>,
    ip_input: String,
    name_input: String,
    err: Option<String>,
    status_msg: Option<String>
}

static NAMES: &'static [&str] = &[
    "Test User",
    "Heavenira",
    "Jackzie",
    "Zeko",
    "@someone"
];

static WINDOW_SIZE: Vec2 = vec2(542.0, 430.0);
impl MainMenu {
    pub fn new(name: Option<String>, ip_addr: Option<SocketAddr>) -> Self {
        let name = name.unwrap_or_else(|| NAMES[random_range(0..NAMES.len())].to_string() );
        Self {
            ip_addr: ip_addr,
            ip_input: "127.0.0.1:3566".to_string(),
            name_input: name,
            err: None,
            status_msg: None
        }
    }

    pub fn ip_addr(&self) -> &Option<SocketAddr> {
        &self.ip_addr
    }

    pub fn name(&self) -> &str {
        &self.name_input
    }

    pub fn err_msg(&self) -> Option<&str> {
        self.err.as_deref()
    }

    pub fn set_err_msg(&mut self, err_msg: Option<String>) {
        self.err = err_msg;
    }

    pub fn set_status_msg(&mut self, status_msg: Option<String>) {
        self.status_msg = status_msg;
    }

    pub fn status_msg(&self) -> Option<&str> {
        self.status_msg.as_deref()
    }

    pub fn connect_info(&self) -> Option<(SocketAddr, String)> {
        if let Some(ip_addr) = &self.ip_addr {
            if let name = &self.name_input {
                return Some((ip_addr.clone(), name.to_string()))
            }
        }
        None
    }

    pub async fn draw(&mut self) {
        clear_background(WHITE);
        widgets::Window::new(hash!(), vec2(screen_width() / 2.0 - WINDOW_SIZE.x / 2.0,
                                           screen_height() / 2.0 - WINDOW_SIZE.y / 2.0,), WINDOW_SIZE)
            .label("Multiplayer Test")
            .titlebar(false)
            .movable(false)
            .ui(&mut *root_ui(), |ui| {
                ui.label(None, "Direct Connect");
                ui.editbox(hash!(), vec2(500., 30.), &mut self.ip_input);
                ui.label(None, "Name");
                ui.editbox(hash!(), vec2(500., 30.), &mut self.name_input);
                if self.name_input.len() > 0 && self.ip_input.len() > 0 {
                    if ui.button(None, "Connect") {
                        if let Some(ip_addr) = self.ip_input.to_socket_addrs().unwrap().next() {
                            self.ip_addr = Some(ip_addr);
                        }
                    }
                }

                if let Some(msg) = self.err_msg() {
                    ui.label(None, msg);
                }
                if let Some(msg) = self.status_msg() {
                    ui.label(None, msg);
                }
            });

    }
}