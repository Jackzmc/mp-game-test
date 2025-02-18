use std::collections::VecDeque;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
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

/// How long of no packets from client do we consider them timed out?
static CLIENT_DISCONNECT_TIMEOUT: Duration = Duration::from_secs(60);

/// How long to wait until we consider packet was lost and resend?
static ACK_TIMEOUT_REPLY: Duration = Duration::from_millis(50);

struct ClientData {
    auth_id: u32,
    addr: SocketAddr,
    last_timestamp: u32,
    seq_number: u16,
    reliable_queue: VecDeque<ReliableEntry>,
    last_packet_time: Instant
}
#[derive(Clone)]
struct ReliableEntry {
    pub seq_id: u16,
    pub packet: Packet,
    pub sent_time: Instant
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
            last_packet_time: Instant::now()
        }
    }
    pub fn mark(&mut self) {
        self.last_packet_time = Instant::now();
    }
    pub fn has_timed_out(&self) -> bool {
       self.last_packet_time.elapsed() > CLIENT_DISCONNECT_TIMEOUT
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
        self.net.check_reliable();
        if let Some((pk, event, addr)) = self.net.next_event() {
            debug!("got event, processing: {:?}", event);
            self.process_event(addr, &pk, event).await;
        }
        for i in 0..MAX_PLAYERS {
            if let Some(client) = &mut self.client_data[i] {
                if client.has_timed_out() {
                    self.disconnect_player(&ClientId::ClientIndex(i as u32), "Timed out".to_string());
                    continue
                }
            }
            if let Some(player) = &mut self.game.players[i] {
                // If change made, update:
                if player.process_actions() {
                    trace!("change made, sending update");
                    let move_event = ServerEvent::Move {
                        client_index: player.client_index,
                        position: player.position,
                    };
                    self.broadcast(move_event);
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
    pub fn disconnect_player(&mut self, client_id: &ClientId, reason: String) {
        if let Some(index) = self.get_client_index(client_id) {
            debug!("disconnecting client index. reason={}", reason);
            self.client_data[index as usize] = None;
            self.game.players[index as usize] = None;
            // TODO: send disconnect packet
        }
    }
    pub fn setup_player(&mut self, addr: SocketAddr, name: String) -> (u32, u32) {
        let client_index = self.game.get_empty_slot().expect("no available player slot");
        // Generate an unique auth id that should be hard to guess
        let auth_id: u32 = random();
        trace!("auth_id={} for new client (id={}) (ip={:?}) (name={})", auth_id, client_index, addr, name);
        let player = PlayerData::new(client_index, name, Position::zero());

        self.game.set_player(client_index, Some(player));

        let client = ClientData::new(auth_id, addr);

        self.client_data[client_index as usize] = Some(client);

        (client_index, auth_id)
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
    pub fn broadcast(&self, event: ServerEvent) -> usize {
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
    pub fn broadcast_reliable(&mut self, event: ServerEvent) -> usize {
        // TODO: make reliable
        let addr_list = self._get_addr_list();
        let len = addr_list.len();
        for addr in addr_list {
            self.send_to_reliable(event.clone(), &ClientId::Addr(addr)).ok();
        }
        len
    }

    /// Send an event to a specific client
    pub fn send_to(&mut self, event: &ServerEvent, addr: SocketAddr) {
        let pk = event.to_packet();
        let buf = pk.as_slice();;
        debug!("EVENT[{}B] {:?} {:?}", buf.len(), addr, event);
        self.net.send(pk, addr).ok();
    }

    /// Sends an event to a specified client, returning Ok(sequence_number) or error if client not found
    pub fn send_to_reliable(&mut self, event: ServerEvent, client_id: &ClientId) -> Result<u16, anyhow::Error> {
        if let Some((_, client)) = self.get_client_mut(client_id) {
            let addr = client.addr.clone();
            let entry = self.net.add_reliable_packet(addr, event.clone());
            debug!("EVENT[{}B] {:?} {:?}", entry.packet.buf_len(), addr, event);
            self.net.send(entry.packet.clone(), addr).ok();
            return Ok(entry.seq_id);
        }
        Err(anyhow!("Could not find client"))
    }

    /// Process a login packet, sending necessary events and registering client/player
    async fn _process_login_packet(&mut self, addr: SocketAddr, packet: &Packet, version: u32, name: String) -> PacketResponse {
        if version != PACKET_PROTOCOL_VERSION {
            warn!("Ignoring login event - invalid protocol version (theirs: {}, ours: {})", version, PACKET_PROTOCOL_VERSION);
            return PacketResponse::Error(anyhow!("invalid protocol version (yours: {}, ours: {})", version, PACKET_PROTOCOL_VERSION));
        }

        // TODO: send_reliable broadcast_reliable

        // Tell client it's auth id and player index
        let (client_index, auth_id) = self.setup_player(addr, name.clone());
        let login_event = ServerEvent::Login {
            client_index,
            auth_id,
        };
        let client_id = ClientId::ClientIndex(client_index);
        self.send_to_reliable(login_event, &client_id).ok();

        // Tell client all connected players
        for i in 0..MAX_PLAYERS {
            if let Some(player) = &self.game.players[i] {
                let event = player.get_spawn_event();
                self.send_to_reliable(event, &client_id).ok();
            }
        }

        // Tell all other clients that this client connected
        let spawn_event = self.game.players[client_index as usize].as_ref().unwrap().get_spawn_event();
        self.broadcast_reliable(spawn_event);

        PacketResponse::Ok
    }

    pub async fn process_event(&mut self, addr: SocketAddr, packet: &Packet, event: ClientEvent) -> PacketResponse {
        let client_id = ClientId::Addr(addr);
        // Verify login separately - as it can't verify auth
        if let ClientEvent::Login { version, name} = event {
            return self._process_login_packet(addr, packet, version, name).await;
        }

        if let Some((client, player)) = self.get_client_player_mut(&client_id) {
            // Verify the client's auth id matches for its ip
            let auth_id = packet.auth_id();
            if client.auth_id != auth_id {
                warn!("dropping packet - got invalid auth id for addr. addr={} auth_id={}", addr, auth_id);
                return PacketResponse::Discarded;
            }
            client.mark(); // update last packet time

            match event {
                // Handled elsewhere
                ClientEvent::Login {..} => unreachable!(),
                ClientEvent::Ack { seq_number } => {
                    // Sequence numbers must be processed in order
                    // so we can only continue if the top entry is ACK'd
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
                },
                ClientEvent::PerformAction { actions } => {
                    trace!("now={} pk.timestamp={} last_timestamp={}", unix_timestamp(), packet.timestamp(), client.last_timestamp);
                    if client.last_timestamp > packet.timestamp() {
                        debug!("discarding packet (last timestamp: {}) (pk timestamp: {})", client.last_timestamp, packet.timestamp());
                        return PacketResponse::Discarded;
                    }
                    client.last_timestamp = packet.timestamp();
                    trace!("got player id={}", player.client_index);
                    player.actions = actions;
                },
                ClientEvent::Disconnect { reason} => {
                    trace!("client disconnect (index={}) (reason={})", player.client_index, reason);
                    let event = ServerEvent::Disconnect {
                        client_index: player.client_index,
                        reason,
                    };
                    self.broadcast_reliable(event);
                }
            }
            return PacketResponse::Ok
        }
        warn!("dropping event - failed to find client for addr {}", addr);
        PacketResponse::Discarded
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