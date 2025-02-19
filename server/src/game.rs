use std::collections::VecDeque;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::thread::sleep;
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

/// How long to sleep when we are in sleep mode
static SLEEP_INTERVAL: Duration = Duration::from_millis(1000);


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
    active_tick_interval: Interval,
    per_tick_duration: Duration,
    tick_count: u8,

    sleep_interval: Option<Interval>
}

impl GameInstance {
    pub fn new(tick_rate: u8) -> Self {
        let ms_per_tick = 1000 / TICK_RATE as u16;
        debug!("tickrate={} ms per tick={}", TICK_RATE, ms_per_tick);
        let mut per_tick_duration = Duration::from_millis(ms_per_tick as u64);
        Self {
            // TODO: make socket
            net: NetServer::new(),
            game: CommonGameInstance::new(),
            client_data: [const { None }; MAX_PLAYERS],

            tick_rate,
            active_tick_interval: interval(per_tick_duration),
            per_tick_duration,
            tick_count: 0,

            sleep_interval: Some(interval(Duration::from_millis(500)))
        }
    }

    /// Overwrites the game tick interval with a specific duration
    /// Set to None to restore the normal set tick rate
    fn set_sleep(&mut self, value: bool) {
        if value {
            if self.in_sleep() { return; } // ignore if already asleep
            debug!("entering sleep ({} ms)", SLEEP_INTERVAL.as_millis());
            self.sleep_interval = Some(interval(SLEEP_INTERVAL));
        } else {
            if !self.in_sleep() { return; } // ignore if already awake
            debug!("waking up from sleep");
            self.sleep_interval = None;
        }
    }
    pub fn in_sleep(&self) -> bool {
        self.sleep_interval.is_some()
    }
    /// TODO:
    /// default state: pull long (ms per tick + some extra slow ms)
    /// if during poll interval detect net activity, wake up

    pub async fn tick(&mut self) {
        // Always process packets sleep or not
        if let Some((pk, event, addr)) = self.net.next_event() {
            debug!("got event, processing: {:?}", event);
            self.process_event(addr, &pk, event).await;
        }

        // Try to sleep if applicable
        if !self.try_sleep().await {
            // Not sleeping - process things
            self.active_tick_interval.tick().await;
            self.process().await;
        }
    }

    /// If we are in sleep mode, sleeps for sleep interval.
    /// Returns true if slept or false if not sleeping (or just woke up)
    async fn try_sleep(&mut self) -> bool {
        if let Some(deep_sleep) = &mut self.sleep_interval {
            deep_sleep.tick().await;
            // Check if there was any activity to wake us
            if self.net.stat().has_activity_within(Duration::from_millis(1000)) || self.game.player_count() > 0 {
                debug!("try_sleep: waking from sleep due to activity");
                self.set_sleep(false);
                return false
            }
            return true
        }
        false
    }

    /// Process packets, player world
    pub async fn process(&mut self) {
        if let Some((pk, event, addr)) = self.net.next_event() {
            debug!("got event, processing: {:?}", event);
            self.process_event(addr, &pk, event).await;
        }
        let mut client_count = 0;
        for i in 0..MAX_PLAYERS {
            if let Some(client) = &mut self.client_data[i] {
                if client.has_timed_out() {
                    self.disconnect_player(&ClientId::ClientIndex(i as u32), "Timed out".to_string());
                    continue
                }
            }
            if let Some(player) = &mut self.game.players[i] {
                // TODO: disconnect but client couint still 1?
                client_count += 1;
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
            let pk_count = self.net.pks_per_interval();
            debug!("tick summary. ticks={} pk_in={}/s pk_out={}/s clients={}", self.tick_count, pk_count.rx, pk_count.tx, client_count);
            self.tick_count = 0;
            // If we haven't seen any network activity then we can sleep
            if !self.net.stat().has_activity_within(Duration::from_millis(30_000)) && self.game.player_count() == 0 {
                debug!("no net activity in 30s and no players, sleeping");
                self.set_sleep(true);
            }
        }
    }


    pub fn disconnect_player(&mut self, client_id: &ClientId, reason: String) {
        if let Some(index) = self.get_client_index(client_id) {
            debug!("disconnecting client index {}. reason={}", index, reason);
            self.client_data[index as usize] = None;
            self.game.players[index as usize] = None;
            // TODO: send disconnect packet
        }
        // If no more players, then we can sleep
        if self.game.player_count() == 0 {
            debug!("no more players, going to sleep");
            self.set_sleep(true);
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

        self.set_sleep(false); // in case we are sleeping (unlikely), unsleep

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
                ClientEvent::Ack {..} | ClientEvent::Login {..} => unreachable!(),

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