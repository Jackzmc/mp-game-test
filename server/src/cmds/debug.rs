use crate::cmds::{CommandArgs, ServerCommand};
use crate::game::GameInstance;

#[derive(Default)]
pub struct Command {}
impl ServerCommand for Command {
    fn run(&self, game: &mut GameInstance, client_index: u32, command: CommandArgs) -> bool {
        let net_stat = game.net.stat();
        let activity_time = net_stat.activity_time_as_secs_f32();
        let pk_count = net_stat.pk_count();
        println!("pks rate in={}/s out={}/s", pk_count.rx, pk_count.rx);
        println!("net activity in[{}s ago] out[{}s ago]",
                 activity_time.rx.unwrap_or("Never".to_string()),
                 activity_time.tx.unwrap_or("Never".to_string()),
        );
        println!("in sleep = {}\t\tuptime = {:.2} min", game.in_sleep(), game.uptime().as_secs_f64() / 60.0);
        true
    }
}