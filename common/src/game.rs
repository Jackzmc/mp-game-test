use std::fmt::{Debug, Formatter};
use bitflags::bitflags;
use int_enum::IntEnum;
use crate::ClientIndex;
use crate::def::{Vector3, MAX_PLAYERS};
use crate::events_server::ServerEvent;

#[derive(Debug)]
pub struct PlayerData{
    // pub client: C,
    pub position: Vector3,
    pub angles: Vector3,
    pub name: String,
    pub client_index: u32,
    pub state: PlayerState,
    pub actions: Action
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
    pub fn new(client_id: u32, name: String, position: Vector3, angles: Vector3) -> Self{
        PlayerData {
            // client: client_data,
            position,
            angles,
            name,
            client_index: client_id,
            state: PlayerState::default(),
            actions: Action::empty()
        }
    }
    pub fn get_spawn_event(&self) -> ServerEvent {
        ServerEvent::PlayerSpawn {
            client_index: self.client_index,
            name: self.name.clone(),
            position: self.position,
            angles: self.angles,
        }
    }

    pub fn process_actions(&mut self) -> bool {
        let mut changed = false;
        if self.actions.contains(Action::Forward) {
            self.position.y -= 1.0;
            if self.position.y < 0.0 {
                self.position.y = 0.0;
            }
            changed = true;
        }
        if self.actions.contains(Action::Backward) {
            self.position.y += 1.0;
            if self.position.y > 1000.0 {
                self.position.y = 1000.0;
            }
            changed = true;
        }
        if self.actions.contains(Action::Left) {
            self.position.x += 1.0;
            if self.position.x < 0.0 {
                self.position.x = 0.0;
            }
            changed = true;
        }
        if self.actions.contains(Action::Right) {
            self.position.x -= 1.0;
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

    pub fn player_count(&self) -> usize {
        let mut count = 0;
        for i in 0..MAX_PLAYERS {
            if self.players[i].is_some() {
                count += 1;
            }
        }
        count
    }

    fn _check_player_id(&self, client_index: u32) {
        assert!(client_index <= self.players.len() as u32, "client index out of bounds");
    }

    pub fn get_empty_slot(&self) -> Option<u32> {
        for i in 0..MAX_PLAYERS {
            if self.players[i].is_none() {
                return Some(i as u32)
            }
        }
        None
    }

    pub fn set_player(&mut self, client_index: u32, player: Option<PlayerData>) {
        self._check_player_id(client_index);
        self.players[client_index as usize] = player;
    }

    pub fn get_player(&self, client_index: u32) -> &Option<PlayerData> {
        self._check_player_id(client_index);
        &self.players[client_index as usize]
    }

    pub fn get_player_mut(&mut self, client_index: u32) -> &mut Option<PlayerData> {
        self._check_player_id(client_index);
        &mut self.players[client_index as usize]
    }
}