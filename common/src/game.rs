use std::fmt::{Debug, Formatter};
use bitflags::bitflags;
use int_enum::IntEnum;
use crate::def::{Position, MAX_PLAYERS};
use crate::events_server::ServerEvent;

#[derive(Debug)]
pub struct PlayerData{
    // pub client: C,
    pub position: Position,
    pub name: String,
    pub client_id: u32,
    pub state: PlayerState
}

pub trait ClientData {
    fn id(&self) -> u32;
}

#[derive(Debug, Default)]
pub struct PlayerState {
    // pub health: f32
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct Action: u32 {
        const Forward = 0x1;
        const Backward = 0x2;
        const Left = 0x4;
        const Right = 0x8;
        const Jump = 0x16;
        const Crouch = 0x32;
        const Walk = 0x64;
        const Run = 0x128;
        const Use = 0x256;
        const LeftClick = 0x512;
        const RightClick = 0x1024;
        const MiddleMouse = 0x2048;
    }
}


impl PlayerData {
    pub fn new(client_id: u32, name: String, position: Position) -> Self{
        PlayerData {
            // client: client_data,
            position,
            name,
            client_id,
            state: PlayerState::default()
        }
    }
    pub fn get_spawn_event(&self) -> ServerEvent {
        ServerEvent::PlayerSpawn {
            client_id: self.client_id,
            name: self.name.clone(),
            position: self.position
        }
    }

    pub fn process_action(&mut self, action: Action) -> bool {
        let mut changed = false;
        if action.contains(Action::Forward) {
            self.position.y -= 1.0;
            if self.position.y < 0.0 {
                self.position.y = 0.0;
            }
            changed = true;
        }
        if action.contains(Action::Backward) {
            self.position.y += 1.0;
            if self.position.y > 1000.0 {
                self.position.y = 1000.0;
            }
            changed = true;
        }
        if action.contains(Action::Left) {
            self.position.x -= 1.0;
            if self.position.x < 0.0 {
                self.position.x = 0.0;
            }
            changed = true;
        }
        if action.contains(Action::Right) {
            self.position.x += 1.0;
            if self.position.x > 1000.0 {
                self.position.x = 1000.0;
            }
            changed = true;
        }

        changed
    }
}

#[derive(Debug)]
pub struct CommonGameInstance {
    pub seq_number: u16,
    pub players: [Option<PlayerData>; MAX_PLAYERS as usize],
    // pub entities: Vec<None>
}

impl CommonGameInstance {
    pub fn new() -> Self {
        Self {
            seq_number: 0,
            players: [const { None }; MAX_PLAYERS as usize],
        }
    }

    fn _check_player_id(&self, client_id: u32) {
        assert!(client_id <= self.players.len() as u32, "client index out of bounds");
    }

    pub fn set_player(&mut self, client_id: u32, player: Option<PlayerData>) -> &PlayerData {
        self._check_player_id(client_id);
        self.players[client_id as usize] = player;
        self.players[client_id as usize].as_ref().unwrap()
    }

    pub fn get_player(&self, client_id: u32) -> &Option<PlayerData> {
        self._check_player_id(client_id);
        &self.players[client_id as usize]
    }

    pub fn get_player_mut(&mut self, client_id: u32) -> &mut Option<PlayerData> {
        self._check_player_id(client_id);
        &mut self.players[client_id as usize]
    }
}