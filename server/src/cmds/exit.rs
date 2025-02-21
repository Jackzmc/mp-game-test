use crate::cmds::{CommandArgs, ServerCommand};
use crate::game::GameInstance;

#[derive(Default)]
pub struct ExitCommand {}
impl ServerCommand for ExitCommand {
    fn run(&self, game: &mut GameInstance, client_index: u32, command: CommandArgs) -> bool {
        game.shutdown();
        true
    }
}