mod game;

use std::time::Instant;
use log::{debug, error, info, trace};
use tokio::net::UdpSocket;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use mp_game_test_common::game::{CommonGameInstance, PlayerData};
use mp_game_test_common::{setup_logger, PacketSerialize};
use rand::random;
use mp_game_test_common::events_client::ClientEvent;
use mp_game_test_common::events_server::ServerEvent;
use mp_game_test_common::packet::Packet;
use crate::game::GameInstance;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();

    let sock = UdpSocket::bind("0.0.0.0:3566").await?;
    let mut buf = [0; 1024];
    info!("server listening at UDP {:?}", sock.local_addr().unwrap());

    let mut game = GameInstance::new(sock);
    loop {
        let (len, addr) = game.socket().recv_from(&mut buf).await?;
        let packet = Packet::from(buf.as_slice());
        trace!("[{:?}] IN n={} {:?}", addr, len, packet.buf().slice(0, len));
        if !packet.is_valid() {
            trace!("[{:?}] INVALID packet, ignoring", addr);
            continue;
        }
        let auth_id = packet.auth_id();
        if let Ok(event) = ClientEvent::from_packet(packet) {
            if let Err(e) = game.process_event(addr, auth_id, event).await {
                error!("[{:?}] Process event failed: {:?}", addr, e);
            }
        }
    }
}
