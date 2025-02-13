use std::time::Instant;
use log::{debug, error, info, trace};
use tokio::net::UdpSocket;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use mp_game_test_common::def::{ClientEvent, Position};
use mp_game_test_common::PacketSerialize;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::filter::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=trace", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let sock = UdpSocket::bind("0.0.0.0:3566").await?;
    let mut buf = [0; 1024];
    info!("server listening at UDP {:?}", sock.local_addr().unwrap());

    let mut last_update = Instant::now();
    loop {
        let (len, addr) = sock.recv_from(&mut buf).await?;
        trace!("[{:?}] IN n={} {:0X}", addr, len, &buf[0..len]);

        let len = sock.send_to(&buf[..len], addr).await?;
        println!("{:?} bytes sent", len);

        if last_update.elapsed().as_secs() > 6 {
            last_update = Instant::now();
            let event = ClientEvent::Move { position: Position::new(0.0, 0.0, 0.0)};
            let packet = event.to_packet();
            debug!("sending update");
            sock.send_to(&packet.as_slice(), addr).await.unwrap();
        }
    }
}
