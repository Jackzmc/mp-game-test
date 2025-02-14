use std::fmt::{Debug, Formatter};
use crate::def::{Position, MAX_PLAYERS};

#[derive(Debug)]
pub struct PlayerData {
    pub position: Position,
    pub name: String,
    pub client_id: u32
}

#[derive(Debug)]
pub struct CommonGameInstance {
    pub players: [Option<PlayerData>; MAX_PLAYERS as usize],
    // pub entities: Vec<None>
}

impl CommonGameInstance {
    pub fn new() -> Self {
        Self {
            players: [const { None }; MAX_PLAYERS as usize],
        }
    }

    pub fn init_player(&mut self, client_id: u32, name: String, position: Position) {
        let pd = PlayerData {
            position: Position::zero(),
            name,
            client_id: client_id
        };
        self.players[client_id as usize] = Some(pd);
    }

    pub fn get_player(&self, client_id: u32) -> &Option<PlayerData> {
        if client_id < 0 || client_id as usize >= self.players.len() {
            panic!("Client index out of bounds");
        }
        &self.players[client_id as usize]
    }

    pub fn get_player_mut(&mut self, client_id: u32) -> &mut Option<PlayerData> {
        if client_id < 0 || client_id as usize >= self.players.len() {
            panic!("Client index out of bounds");
        }
        &mut self.players[client_id as usize]
    }
}