use bitflags::bitflags;
use crate::cmds::exit::ExitCommand;
use crate::cmds::help::HelpCommand;
use crate::cmds::status::StatusCommand;
use crate::game::GameInstance;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct CmdFlag: u32 {
        const Hidden = 0x1;
        const ClientCanExecute = 0x2;
    }
}

/// Converts a string of "cmd arg1 arg2" into ("cmd", &["arg1", "arg2"])
pub fn parse_args(line: &str) -> (String, Vec<String>) {
    let mut split = line.split(" ");
    let command: String = split.next().expect("command is empty").into();
    let args = split.into_iter().map(|s| s.to_owned()).collect();
    (command, args)
}

pub trait ServerCommand {
    // fn aliases() -> Vec<String>;
    fn run(&self, game: &mut GameInstance, client_index: u32, command: &str, args: &[String]) -> bool;
}

mod help;
mod status;
mod exit;

pub fn register_commands(game: &mut GameInstance) {
    game.reg_cmd("help", Box::new(HelpCommand::default()));
    game.reg_cmd("status", Box::new(StatusCommand::default()));
    game.reg_cmd("exit", Box::new(ExitCommand::default()));
}