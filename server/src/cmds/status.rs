use crate::cmds::{CommandArgs, ServerCommand};
use crate::game::GameInstance;

#[derive(Default)]
pub struct StatusCommand {}
impl ServerCommand for StatusCommand {
    fn run(&self, game: &mut GameInstance, client_index: u32, command: CommandArgs) -> bool {
        println!(
            "{0: <6} | {1: <11} | {2: <32}",
            "index", "auth_id", "name"
        );
        game.for_all_players(|index, client, player| {
            println!(
                "{0: <6} | {1: <11} | {2: <32}",
               index, client.auth_id, player.name
            );
        });
        true
    }
}