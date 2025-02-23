use std::net::SocketAddr;
use log::{debug, trace, warn};
use macroquad::camera::Camera3D;
use macroquad::math::Vec3;
use mp_game_test_common::events_client::ClientEvent;
use mp_game_test_common::game::{Action, CommonGameInstance, PlayerData};
use mp_game_test_common::events_server::ServerEvent;
use mp_game_test_common::{PacketSerialize, PACKET_PROTOCOL_VERSION};
use mp_game_test_common::def::Vector3;
use crate::network::NetClient;

pub struct GameInstance {
    pub game: CommonGameInstance,
    pub net: Option<NetClient>,
    pub cam: GameCamera,
    pub local_player: LocalPlayer,
    client_id: Option<u32>,
    auth_id: Option<u32>,
    actions: Action
}
#[derive(Default)]
pub struct GameCamera {
    pub camera: Camera3D,
    pub rotation: Vec3,
}
#[derive(Default)]
pub struct LocalPlayer {
    pub front: Vec3
}
impl GameCamera {
    pub fn set_target(&mut self, target: Vec3) {
        self.camera.target = target;
    }
}
impl GameInstance {
    pub fn new() -> Self {
        Self {
            game: CommonGameInstance::new(),
            cam: GameCamera::default(),
            local_player: LocalPlayer::default(),
            net: None,
            client_id: None,
            auth_id: None,
            actions: Action::empty()
        }
    }
    pub fn connect(&mut self, addr: SocketAddr, name: String) -> Result<(), String> {
        if self.net.is_some() {
            return Err("Already connected".to_string());
        }
        self.net = Some(NetClient::new(addr));
        let event = ClientEvent::Login {
            version: PACKET_PROTOCOL_VERSION,
            name: name
        };
        // This should never really fail - is just a channel to another thread
        self.send(&event).map(|_| ())
    }
    pub fn is_connected(&self) -> bool {
        self.net.is_some()
    }
    pub(crate) fn net(&self) -> &NetClient {
        self.net.as_ref().expect("not connected")
    }
    pub(crate) fn net_mut(&mut self) -> &mut NetClient {
        self.net.as_mut().expect("not connected")
    }
    pub fn client_id(&self) -> Option<u32> {
        self.client_id.as_ref().cloned()
    }
    pub fn auth_id(&self) -> Option<u32> {
        self.auth_id.as_ref().cloned()
    }
    pub fn is_authenticated(&self) -> bool {
        self.is_connected() && self.auth_id.is_some()
    }
    pub fn _on_login(&mut self, client_id: u32, auth_id: u32) {
        self.client_id = Some(client_id);
        self.auth_id = Some(auth_id);
        debug!("LOGIN client id = {}, auth_id = {}", client_id, auth_id);
    }

    /// Returns our player instance
    pub fn player(&self) -> Option<&PlayerData> {
        self.client_id.and_then(|client_id| self.game.players[client_id as usize].as_ref())
    }

    pub fn player_mut(&mut self) -> Option<&mut PlayerData> {
        self.client_id.and_then(|client_id| self.game.players[client_id as usize].as_mut())
    }

    pub fn disconnect<S>(&mut self, reason: S) where S: Into<String> {
        trace!("disconnect triggered, sending");
        let event = ClientEvent::Disconnect {
            reason: reason.into()
        };
        self.send(&event).ok();
        let net = self.net.take().unwrap();
        trace!("ending net threads");
        net.end();
    }

    pub fn has_action(&self, action: Action) -> bool {
        self.actions.contains(action)
    }

    pub fn set_action(&mut self, action: Action, value: bool) -> Result<(), String> {
        self.actions.set(action, value);
        let player = self.player().ok_or(format!("player not active"))?;
        let event = ClientEvent::PerformAction {
            actions: self.actions,
            angles: player.angles,
        };
        self.send(&event)
    }

    pub fn send(&self, event: &ClientEvent) -> Result<(), String> {
        let mut pk = event.to_packet_builder()
            .with_auth_id(self.auth_id.unwrap_or(0));
        assert!(event.get_packet_type() == 0x1 || self.auth_id.is_some(), "non-login event {:?} but no auth id {:?}", event, self.auth_id);
        self.net().send(pk.finalize())
    }

    pub fn process_event(&mut self, event: ServerEvent) {
        match event {
            ServerEvent::Login { client_index: client_id, auth_id } => {
                // Check if we are already logged in
                if let Some(current_auth_id) = self.auth_id {
                    // If it's the same thing - disregard
                    if auth_id != current_auth_id {
                        warn!("received login data when already authenticated. current_auth_id={} auth_id={}", current_auth_id, auth_id);
                    }
                    return;
                }
                self._on_login(client_id, auth_id);
            }
            ServerEvent::PlayerSpawn { client_index: client_id, name, position, angles } => {
                trace!("new player \"{}\" client id {}", name, client_id);
                let player = PlayerData::new(client_id, name, position, angles);
                self.game.set_player(client_id, Some(player));
            }
            ServerEvent::Move { client_index: client_id, position, angles, velocity } => {
                if let Some(player) = self.game.get_player_mut(client_id) {
                    trace!("move player {} | {:?} -> {:?}", client_id, player.position, position);
                    player.position = position;
                    player.angles = angles;
                    // player.velocity = velocity;
                }
            }
            ServerEvent::Disconnect { client_index, reason } => {
                self.game.set_player(client_index, None);
            }
        }
    }
}
