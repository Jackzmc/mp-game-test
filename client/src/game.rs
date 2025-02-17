use std::net::SocketAddr;
use log::{debug, trace};
use mp_game_test_common::events_client::ClientEvent;
use mp_game_test_common::game::{Action, CommonGameInstance, PlayerData};
use mp_game_test_common::events_server::ServerEvent;
use mp_game_test_common::{PacketSerialize, PACKET_PROTOCOL_VERSION};
use crate::network::NetClient;

pub struct GameInstance {
    pub game: CommonGameInstance,
    pub net: Option<NetClient>,
    client_id: Option<u32>,
    auth_id: Option<u32>,
    actions: Action
}
impl GameInstance {
    pub fn new() -> Self {
        Self {
            game: CommonGameInstance::new(),
            net: None,
            client_id: None,
            auth_id: None,
            actions: Action::empty()
        }
    }
    pub fn connect(&mut self, addr: SocketAddr) {
        self.net = Some(NetClient::new(addr))
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
        self.auth_id.is_some()
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

    pub fn login(&self, name: String) -> Result<(), String> {
        let event = ClientEvent::Login {
            version: PACKET_PROTOCOL_VERSION,
            name: name
        };
        self.send(&event).map(|_| ())
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
        let event = ClientEvent::PerformAction {
            actions: self.actions
        };
        self.send(&event)
    }

    pub fn send(&self, event: &ClientEvent) -> Result<(), String> {
        let mut pk = event.to_packet_builder()
            .with_auth_id(self.auth_id.unwrap_or(0));
        assert!(event.get_packet_type() == 0x1 || self.auth_id.is_some(), "non-login event {:?} but no auth id {:?}", event, self.auth_id);
        let pk = pk.finalize();
        self.net().send(pk)
    }

    pub fn process_event(&mut self, event: ServerEvent) {
        match event {
            ServerEvent::Login { client_index: client_id, auth_id } => {
                assert_eq!(self.is_authenticated(), false, "received login data when already authenticated");
                self._on_login(client_id, auth_id);
            }
            ServerEvent::PlayerSpawn { client_index: client_id, name, position } => {
                trace!("new player \"{}\" client id {}", name, client_id);
                let player = PlayerData::new(client_id, name, position);
                self.game.set_player(client_id, Some(player));
            }
            ServerEvent::Move { client_index: client_id, position } => {
                if let Some(player) = self.game.get_player_mut(client_id) {
                    trace!("move player {} | {:?} -> {:?}", client_id, player.position, position);
                    player.position = position;
                }
            }
            ServerEvent::Disconnect { client_index, reason } => {
                self.game.set_player(client_index, None);
            }
        }
    }
}
