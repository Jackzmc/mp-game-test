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
use mp_game_test_common::{PacketSerialize, PACKET_PROTOCOL_VERSION};
use mp_game_test_common::def::{Position, MAX_PLAYERS};

struct ClientData {
    auth_id: u32,
    addr: SocketAddr,
}
pub struct GameInstance {
    socket: UdpSocket,
    game: CommonGameInstance,
    client_auth_map: [Option<ClientData>; MAX_PLAYERS],
}
impl GameInstance {
    pub fn new(socket: UdpSocket) -> Self {
        Self {
            // TODO: make socket
            socket,
            game: CommonGameInstance::new(),
            client_auth_map: [const { None }; MAX_PLAYERS],
        }
    }

    pub fn socket(&self) -> &UdpSocket {
        &self.socket
    }
    pub fn authorize_player(&mut self, addr: SocketAddr, name: String) -> (u32, u32) {
        for i in 0..MAX_PLAYERS {
            if self.game.players[i].is_none() {
                let auth_id = random();
                trace!("auth_id={} for new client (id={}) (ip={:?}) (name={})", auth_id, i, addr, name);
                self.game.init_player(i as u32, name, Position::zero());

                // Generate an auth id
                let client = ClientData {
                    auth_id: auth_id,
                    addr,
                };

                self.client_auth_map[i] = Some(client);

                return (i as u32, auth_id);
            }
        }
        panic!("No available player slots");
    }

    pub fn get_client_id_from_auth_id(&self, auth_id: u32) -> Option<u32> {
        for i in 0..MAX_PLAYERS {
            if let Some(client) = &self.client_auth_map[i] {
                trace!("{} auth_id={} input={}", i, client.auth_id, auth_id);
                if auth_id == client.auth_id {
                    return Some(i as u32);
                }
            }
        }
        None
    }

    pub async fn broadcast(&self, event: ServerEvent) -> std::io::Result<usize> {
        let pk = event.to_packet();
        let buf = pk.as_slice();;

        let mut count = 0;
        for i in 0..MAX_PLAYERS {
            if let Some(client) = &self.client_auth_map[i] {
                self.socket.send_to(buf, client.addr).await.ok();
                // TODO: handle failures on specific client?, instead of silent?
            }
        }
        debug!("EVENT[{}B] BROADCAST[{}] {:?}", buf.len(), count, event);
        trace!("{:?}", pk.buf());
        Ok(count)
    }

    pub async fn send_to(&self, event: ServerEvent, addr: SocketAddr) -> std::io::Result<usize> {
        let pk = event.to_packet();
        let buf = pk.as_slice();;
        debug!("EVENT[{}B] {:?} {:?}", buf.len(), addr, event);
        trace!("{:?}", pk.buf());
        self.socket.send_to(buf, addr).await
    }

    pub fn get_player_mut(&mut self, auth_id: u32) -> Result<&mut PlayerData, anyhow::Error> {
        let client_id = self.get_client_id_from_auth_id(auth_id).ok_or(anyhow!("invalid auth id"))?;
        assert!(client_id < MAX_PLAYERS as u32, "client id is over MAX_PLAYERS");
        let player = self.game.players[client_id as usize].as_mut();
        player.ok_or(anyhow!("invalid client index"))
    }

    pub async fn process_event(&mut self, addr: SocketAddr, auth_id: u32, event: ClientEvent) -> Result<(), anyhow::Error> {
        match event {
            ClientEvent::Login { version, name } => {
                // auth_id is 0 / unused
                if version != PACKET_PROTOCOL_VERSION {
                    warn!("Ignoring login event - invalid protocol version (theirs: {}, ours: {})", version, PACKET_PROTOCOL_VERSION);
                    return Err(anyhow!("invalid protocol version (yours: {}, ours: {})", version, PACKET_PROTOCOL_VERSION));
                }

                // Tell client it's auth id and player index
                let (client_id, auth_id) = self.authorize_player(addr, name.clone());
                let login_event = ServerEvent::Login {
                    client_id,
                    auth_id,
                };
                self.send_to(login_event, addr).await?;

                // Tell all clients a client connected
                let spawn_event = ServerEvent::PlayerSpawn {
                    client_id,
                    name,
                    position: Position::zero(),
                };
                self.broadcast(spawn_event).await?;
            }
            ClientEvent::Move { position } => {
                trace!("getting player");
                let player = self.get_player_mut(auth_id)?;
                trace!("got player id={}", player.client_id);
                player.position = position;
                let move_event = ServerEvent::Move {
                    client_id: player.client_id,
                    position,
                };
                trace!("sending update");
                self.broadcast(move_event).await?;
            }
        }
        Ok(())
    }
}
