use std::collections::VecDeque;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::time::Duration;
use anyhow::anyhow;
use log::{debug, trace, warn};
use rand::random;
use tokio::net::UdpSocket;
use tokio::time::{interval, Interval};
use mp_game_test_common::events_client::ClientEvent;
use mp_game_test_common::game::{Action, CommonGameInstance, PlayerData};
use mp_game_test_common::events_server::ServerEvent;
use mp_game_test_common::packet::{Packet, PacketBuilder};
use mp_game_test_common::{unix_timestamp, PacketSerialize, PACKET_PROTOCOL_VERSION};
use mp_game_test_common::def::{Position, MAX_PLAYERS};
use crate::network::{NetServer, OutPacket};
use crate::TICK_RATE;

struct ClientData {
    auth_id: u32,
    addr: SocketAddr,
    last_timestamp: u32,
    seq_number: u16,
    reliable_queue: VecDeque<ReliableEntry>,
}
struct ReliableEntry {
    seq_id: u16,
    event: ServerEvent
}

enum ClientId {
    AuthId(u32),
    Addr(SocketAddr),
    ClientIndex(u32)
}
impl ClientData {
    pub fn new(auth_id: u32, addr: SocketAddr) -> Self {
        ClientData {
            auth_id,
            addr,
            last_timestamp: unix_timestamp(),
            seq_number: 0,
            reliable_queue: VecDeque::new(),
        }
    }
    pub fn add_reliable_packet(&mut self, event: ServerEvent) -> u16 {
        let seq = self.seq_number;
        let entry = ReliableEntry {
            seq_id: seq,
            event,
        };
        self.reliable_queue.push_back(entry);
        self.seq_number += 1;
        seq
    }
}
pub struct GameInstance {
    pub net: NetServer,
    game: CommonGameInstance,
    client_data: [Option<ClientData>; MAX_PLAYERS],

    tick_rate: u8,
    tick_interval: Interval,
    tick_count: u8,
}

impl GameInstance {
    pub fn new(tick_rate: u8) -> Self {
        let ms_per_tick = 1000 / TICK_RATE as u16;
        debug!("tickrate={} ms per tick={}", TICK_RATE, ms_per_tick);
        let mut interval = interval(Duration::from_millis(ms_per_tick as u64));
        Self {
            // TODO: make socket
            net: NetServer::new(),
            game: CommonGameInstance::new(),
            client_data: [const { None }; MAX_PLAYERS],

            tick_rate,
            tick_interval: interval,
            tick_count: 0
        }
    }

    pub async fn tick(&mut self) {
        self.tick_interval.tick().await;
        if let Some((pk, event, addr)) = self.net.next_event() {
            debug!("got event, processing: {:?}", event);
            self.process_event(addr, &pk, event).await;
        }
        for i in 0..MAX_PLAYERS {
            if let Some(player) = &mut self.game.players[i] {
                // If change made, update:
                if player.process_actions() {
                    trace!("change made, sending update");
                    let move_event = ServerEvent::Move {
                        client_id: player.client_id,
                        position: player.position,
                    };
                    self.broadcast(move_event).await;
                }

            }
        }
        self.tick_count += 1;
        if self.tick_count == self.tick_rate {
            let (tx_s,rx_s) = self.net.stats();
            debug!("{} ticks ran. packet count -  tx={} rx={}", self.tick_count, tx_s, rx_s);

            self.tick_count = 0;
        }
    }
    pub fn authorize_player(&mut self, addr: SocketAddr, name: String) -> (u32, u32) {
        for i in 0..MAX_PLAYERS {
            if self.game.players[i].is_none() {
                // Generate an unique auth id that should be hard to guess
                let auth_id: u32 = random();
                trace!("auth_id={} for new client (id={}) (ip={:?}) (name={})", auth_id, i, addr, name);
                let player = PlayerData::new(i as u32, name, Position::zero());

                self.game.set_player(i as u32, Some(player));

                let client = ClientData::new(auth_id, addr);

                self.client_data[i] = Some(client);

                return (i as u32, auth_id);
            }
        }
        panic!("No available player slots");
    }

    pub fn for_all_players<F>(&self, func: F) where F: Fn(u32, &ClientData) {
        for i in 0..MAX_PLAYERS {
            if let Some(client) = &self.client_data[i] {
                func(i as u32, client);
            }
        }
    }

    pub fn get_client(&self, client_id: &ClientId) -> Option<(u32, &ClientData)> {
        for i in 0..MAX_PLAYERS {
            if let Some(client) = &self.client_data[i] {
                if let ClientId::AuthId(auth_id) = client_id {
                    if *auth_id == client.auth_id {
                        return Some((i as u32, client));
                    }
                } else if let ClientId::ClientIndex(client_index) = client_id {
                    if *client_index == i as u32 {
                        return Some((i as u32, client));
                    }
                } else if let ClientId::Addr(addr) = client_id {
                    if *addr == client.addr {
                        return Some((i as u32, client));
                    }
                }
            }
        }
        None
    }

    pub fn get_client_index(&self, client_id: &ClientId) -> Option<u32> {
        if let ClientId::ClientIndex(index) = client_id {
            return Some(*index);
        }
        for i in 0..MAX_PLAYERS {
            if let Some(client) = &self.client_data[i] {
                if let ClientId::AuthId(auth_id) = client_id {
                    if *auth_id == client.auth_id {
                        return Some(i as u32);
                    }
                } else if let ClientId::ClientIndex(client_index) = client_id {
                    if *client_index == i as u32 {
                        return Some(i as u32);
                    }
                } else if let ClientId::Addr(addr) = client_id {
                    if *addr == client.addr {
                        return Some(i as u32);
                    }
                }
            }
        }
        None
    }

    pub fn get_client_mut(&mut self, client_id: &ClientId) -> Option<(u32, &mut ClientData)> {
        let client_index = self.get_client_index(client_id)?;
        let client = self.client_data[client_index as usize].as_mut().unwrap();
        Some((client_index, client))
    }

    pub fn get_client_player_mut(&mut self, client_id: &ClientId) -> Option<(&mut ClientData, &mut PlayerData)> {
        let client_index = self.get_client_index(client_id)?;
        let client = self.client_data[client_index as usize].as_mut().expect("auth id client index mismatch for client data");
        let player = self.game.players[client_index as usize].as_mut().expect("auth id client index mismatch for player data");
        Some((client, player))
    }

    pub fn get_client_player(&self, client_id: &ClientId) -> Option<(&ClientData, &PlayerData)> {
        let client_index = self.get_client_index(client_id)?;
        let client = self.client_data[client_index as usize].as_ref().expect("auth id client index mismatch for client data");
        let player = self.game.players[client_index as usize].as_ref().expect("auth id client index mismatch for player data");
        Some((client, player))
        // if let Some((index, client)) = self.get_client_mut(client_id) {
        //     let player = self.game.players[index as usize].as_mut().unwrap();
        //     return Some((client, player))
        // }
        // None
    }

    // fn get_client_player_mut(&mut self, auth_id: u32) -> Option<(&mut ClientData, &mut PlayerData)> {
    //     let client_id = self.get_client_id_from_auth_id(auth_id)?;
    //     let client = self.client_data[client_id as usize].as_mut().expect("auth id client index mismatch for client data");
    //     let player = self.game.players[client_id as usize].as_mut().expect("auth id client index mismatch for player data");
    //     Some((client, player))
    // }

    fn _get_addr_list(&self) -> Vec<SocketAddr> {
        let mut addr_list = Vec::new();
        for i in 0..MAX_PLAYERS {
            if let Some(client) = &self.client_data[i] {
                addr_list.push(client.addr.clone());
                // TODO: handle failures on specific client?, instead of silent?
            }
        }
        addr_list
    }

    /// Sends an event to all clients
    pub async fn broadcast(&self, event: ServerEvent) -> usize {
        let pk = event.to_packet();
        let buf = pk.as_slice();;

        // TODO: use filter/map instead
        let addr_list = self._get_addr_list();
        let len = addr_list.len();
        debug!("EVENT[{}B] BROADCAST[{}] {:?}", buf.len(), len, event);
        self.net.send_multiple(pk, addr_list).ok();
        len
    }

    // Sends a broadcast that must be ACK by all clients
    // No other reliable events will be submitted (on a client basis) until the previous one was ACK
    pub async fn broadcast_reliable(&mut self, event: ServerEvent) -> usize {
        // TODO: make reliable
        let addr_list = self._get_addr_list();
        let len = addr_list.len();
        for addr in addr_list {
            if let Some((_, client)) = self.get_client_mut(&ClientId::Addr(addr)) {
                let seq_id = client.add_reliable_packet(event.clone());
                let addr = client.addr.clone();
                self.send_to(&event, addr).await;
            }
        }
        len
    }

    /// Send an event to a specific client
    pub async fn send_to(&mut self, event: &ServerEvent, addr: SocketAddr) {
        let pk = event.to_packet();
        let buf = pk.as_slice();;
        debug!("EVENT[{}B] {:?} {:?}", buf.len(), addr, event);
        self.net.send(pk, addr).ok();
    }

    /// Sends an event to a specified client, returning Ok(sequence_number) or error if client not found
    pub async fn send_to_reliable(&mut self, event: ServerEvent, client_id: &ClientId) -> Result<u16, anyhow::Error> {
        if let Some((_, client)) = self.get_client_mut(client_id) {
            let seq_id = client.add_reliable_packet(event.clone());
            let addr = client.addr.clone();
            self.send_to(&event, addr).await;
            return Ok(seq_id);
        }
        Err(anyhow!("Could not find client"))
    }

    pub async fn process_event(&mut self, addr: SocketAddr, packet: &Packet, event: ClientEvent) -> PacketResponse {
        match event {
            ClientEvent::Ack { seq_number } => {
                if let Some((client, player)) = self.get_client_player_mut(&ClientId::Addr(addr)) {
                    if let Some(top) = client.reliable_queue.get(0) {
                        if top.seq_id == seq_number {
                            client.reliable_queue.pop_front();
                            trace!("ACK seq={} acknowledged", seq_number);
                            // TODO: somehow next packet in reliable queue gets resent?
                        } else {
                            debug!("mismatch ACK. incoming={} top={}", seq_number, top.seq_id);
                        }
                    } else {
                        debug!("stray ACK, no reliable packet in queue. ignoring")
                    };
                }
                todo!();
            }
            ClientEvent::Login { version, name } => {
                // auth_id is 0 / unused
                if version != PACKET_PROTOCOL_VERSION {
                    warn!("Ignoring login event - invalid protocol version (theirs: {}, ours: {})", version, PACKET_PROTOCOL_VERSION);
                    return PacketResponse::Error(anyhow!("invalid protocol version (yours: {}, ours: {})", version, PACKET_PROTOCOL_VERSION));
                }

                // TODO: send_reliable broadcast_reliable

                // Tell client it's auth id and player index
                let (client_index, auth_id) = self.authorize_player(addr, name.clone());
                let login_event = ServerEvent::Login {
                    client_index,
                    auth_id,
                };
                let client_id = ClientId::ClientIndex(client_index);
                self.send_to_reliable(login_event, &client_id).await.ok();

                // Tell client all connected players
                for i in 0..MAX_PLAYERS {
                    if let Some(player) = &self.game.players[i] {
                        let event = player.get_spawn_event();
                        self.send_to_reliable(event, &client_id).await.ok();
                    }
                }

                // Tell all other clients that this client connected
                let spawn_event = self.game.players[client_index as usize].as_ref().unwrap().get_spawn_event();
                self.broadcast_reliable(spawn_event).await;
            }
            ClientEvent::PerformAction { actions } => {
                let client_id = ClientId::AuthId(packet.auth_id());
                if let Some((client, player)) = self.get_client_player_mut(&client_id) {
                    trace!("now={} pk.timestamp={} last_timestamp={}", unix_timestamp(), packet.timestamp(), client.last_timestamp);
                    if client.last_timestamp > packet.timestamp() {
                        debug!("discarding packet (last timestamp: {}) (pk timestamp: {})", client.last_timestamp, packet.timestamp());
                        return PacketResponse::Discarded;
                    }
                    client.last_timestamp = packet.timestamp();
                    trace!("got player id={}", player.client_id);
                    player.actions = actions;
                }
            }
        }
        PacketResponse::Ok
    }
}

pub(crate) enum PacketResponse {
    // Packet was processed successfully
    Ok,
    // Packet was discarded. This occurs when it's outdated (an earlier packet is seceded by a newer)
    Discarded,
    // An error happened while processing
    Error(anyhow::Error),
}