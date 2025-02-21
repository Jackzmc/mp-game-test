use crate::cmds::ServerCommand;
use crate::game::GameInstance;

#[derive(Default)]
pub struct HelpCommand {}
impl ServerCommand for HelpCommand {
    fn run(&self, game: &mut GameInstance, client_index: u32, command: &str, args: &[String]) -> bool {
        for cmd in game.get_cmds() {
            println!("{}", cmd);
        }
        true
    }
}