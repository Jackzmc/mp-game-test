use std::collections::VecDeque;
use std::io::ErrorKind;
use std::net::SocketAddr;
use anyhow::anyhow;
use log::{debug, trace, warn};
use rand::random;
use tokio::net::UdpSocket;
use mp_game_test_common::events_client::ClientEvent;
use mp_game_test_common::game::{CommonGameInstance, PlayerData};
use mp_game_test_common::events_server::ServerEvent;
use mp_game_test_common::packet::{Packet, PacketBuilder};
use mp_game_test_common::{unix_timestamp, PacketSerialize, PACKET_PROTOCOL_VERSION};
use mp_game_test_common::def::{Position, MAX_PLAYERS};

struct ClientData {
    auth_id: u32,
    addr: SocketAddr,
    last_timestamp: u32
}
struct ReliableEntry {
    seq_id: u16,
    event: ServerEvent
}
pub struct GameInstance {
    socket: UdpSocket,
    game: CommonGameInstance,
    client_data: [Option<ClientData>; MAX_PLAYERS],
    reliable_queue: VecDeque<ReliableEntry>,
}
impl GameInstance {
    pub fn new(socket: UdpSocket) -> Self {
        Self {
            // TODO: make socket
            socket,
            game: CommonGameInstance::new(),
            client_data: [const { None }; MAX_PLAYERS],
            reliable_queue: VecDeque::new()
        }
    }

    pub fn socket(&self) -> &UdpSocket {
        &self.socket
    }
    pub fn authorize_player(&mut self, addr: SocketAddr, name: String) -> (u32, u32) {
        for i in 0..MAX_PLAYERS {
            if self.game.players[i].is_none() {
                // Generate an unique auth id that should be hard to guess
                let auth_id: u32 = random();
                trace!("auth_id={} for new client (id={}) (ip={:?}) (name={})", auth_id, i, addr, name);
                let player = PlayerData::new(i as u32, name, Position::zero());

                self.game.set_player(i as u32, Some(player));

                let client = ClientData {
                    auth_id: auth_id,
                    addr,
                    last_timestamp: unix_timestamp(),
                };

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

    pub fn get_client_id_from_auth_id(&self, auth_id: u32) -> Option<u32> {
        for i in 0..MAX_PLAYERS {
            if let Some(client) = &self.client_data[i] {
                if auth_id == client.auth_id {
                    return Some(i as u32);
                }
            }
        }
        None
    }


    pub async fn broadcast(&self, event: ServerEvent) -> usize {
        let pk = event.to_packet();
        let buf = pk.as_slice();;

        let mut count = 0;
        for i in 0..MAX_PLAYERS {
            if let Some(client) = &self.client_data[i] {
                count += 1;
                if let Err(e) = self.socket.send_to(buf, client.addr).await {
                    warn!("broadcast error to {:?}: {}", client.addr, e);
                }
                // TODO: handle failures on specific client?, instead of silent?
            }
        }
        debug!("EVENT[{}B] BROADCAST[{}] {:?}", buf.len(), count, event);
        trace!("{:?}", pk.buf());
        count
    }

    pub async fn broadcast_reliable(&self, event: ServerEvent) {
        // TODO: make reliable
        self.broadcast(event).await;
    }

    pub async fn send_to(&self, event: ServerEvent, addr: SocketAddr) -> std::io::Result<usize> {
        let pk = event.to_packet();
        let buf = pk.as_slice();;
        debug!("EVENT[{}B] {:?} {:?}", buf.len(), addr, event);
        trace!("{:?}", pk.buf());
        self.socket.send_to(buf, addr).await
    }

    pub async fn send_to_reliable(&self, event: ServerEvent, addr: SocketAddr) {
        // TODO: make reliable
        self.send_to(event, addr).await.ok();
    }

    pub fn get_player_mut(&mut self, auth_id: u32) -> Result<&mut PlayerData, anyhow::Error> {
        let client_id = self.get_client_id_from_auth_id(auth_id).ok_or(anyhow!("invalid auth id"))?;
        let player = self.game.players[client_id as usize].as_mut();
        player.ok_or(anyhow!("invalid client index"))
    }

    pub fn get_client(&self, auth_id: u32) -> Result<&ClientData, anyhow::Error> {
        let client_id = self.get_client_id_from_auth_id(auth_id).ok_or(anyhow!("invalid auth id"))?;
        let client = self.client_data[client_id as usize].as_ref();
        client.ok_or(anyhow!("invalid client index"))
    }

    fn get_client_player(&mut self, auth_id: u32) -> Option<(&mut ClientData, &mut PlayerData)> {
        let client_id = self.get_client_id_from_auth_id(auth_id)?;
        let client = self.client_data[client_id as usize].as_mut().expect("auth id client index mismatch for client data");
        let player = self.game.players[client_id as usize].as_mut().expect("auth id client index mismatch for player data");
        Some((client, player))
    }

    pub async fn process_event(&mut self, addr: SocketAddr, packet: &Packet, event: ClientEvent) -> PacketResponse {
        match event {
            ClientEvent::Login { version, name } => {
                // auth_id is 0 / unused
                if version != PACKET_PROTOCOL_VERSION {
                    warn!("Ignoring login event - invalid protocol version (theirs: {}, ours: {})", version, PACKET_PROTOCOL_VERSION);
                    return PacketResponse::Error(anyhow!("invalid protocol version (yours: {}, ours: {})", version, PACKET_PROTOCOL_VERSION));
                }

                // TODO: send_reliable broadcast_reliable

                // Tell client it's auth id and player index
                let (client_id, auth_id) = self.authorize_player(addr, name.clone());
                let login_event = ServerEvent::Login {
                    client_id,
                    auth_id,
                };
                self.send_to_reliable(login_event, addr).await;

                // Tell client all connected players
                for i in 0..MAX_PLAYERS {
                    if let Some(player) = &self.game.players[i] {
                        let event = player.get_spawn_event();
                        self.send_to_reliable(event, addr).await;
                    }
                }

                // Tell all other clients that this client connected
                let spawn_event = self.game.players[client_id as usize].as_ref().unwrap().get_spawn_event();
                self.broadcast_reliable(spawn_event).await;
            }
            ClientEvent::PerformAction { action } => {
                if let Some((client, player)) = self.get_client_player(packet.auth_id()) {
                    trace!("now={} pk.timestamp={} last_timestamp={}", unix_timestamp(), packet.timestamp(), client.last_timestamp);
                    if client.last_timestamp > packet.timestamp() {
                        debug!("discarding packet (last timestamp: {}) (pk timestamp: {})", client.last_timestamp, packet.timestamp());
                        return PacketResponse::Discarded;
                    }
                    client.last_timestamp = packet.timestamp();
                    trace!("got player id={}", player.client_id);
                    // Process action, and if there was a change - send Move Event
                    if player.process_action(action) {
                        trace!("change made, sending update");
                        let move_event = ServerEvent::Move {
                            client_id: player.client_id,
                            position: player.position,
                        };
                        self.broadcast(move_event).await;
                    }
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