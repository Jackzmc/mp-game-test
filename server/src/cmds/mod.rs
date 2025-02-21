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

pub trait ServerCommand {
    // fn aliases() -> Vec<String>;
    fn run(&self, game: &mut GameInstance, client_index: u32, command: CommandArgs) -> bool;
}

pub struct CommandArgs {
    _name: String,
    _args: Vec<String>,
}
impl CommandArgs {

    pub fn from_line(line: &str) -> Self {
        let mut split = line.split(" ");
        let command: String = split.next().expect("command is empty").into();
        let args = split.into_iter().map(|s| s.to_owned()).collect();
        CommandArgs { _name: command, _args: args }
    }
    pub fn name(&self) -> &str {
       self._name.as_str()
    }
    pub fn args(&self) -> usize {
        self._args.len()
    }

    pub fn get_arg_str(&self, arg_index: usize) -> Option<&str> {
        self._args.get(arg_index).map(String::as_str)
    }

    pub fn get_arg_int(&self, arg_index: usize) -> Option<usize> {
        self._args.get(arg_index).map(|s| s.parse::<usize>().ok()).flatten()
    }

    pub fn get_arg_float(&self, arg_index: usize) -> Option<f32> {
        self._args.get(arg_index).map(|s| s.parse::<f32>().ok()).flatten()
    }

    pub fn get_arg_any<T: std::str::FromStr>(&self, arg_index: usize) -> Option<T> {
        self._args.get(arg_index).map(|s| s.parse::<T>().ok()).flatten()
    }
}

mod help;
mod status;
mod exit;
mod debug;

pub fn register_commands(game: &mut GameInstance) {
    game.reg_cmd("help", Box::new(HelpCommand::default()));
    game.reg_cmd("status", Box::new(StatusCommand::default()));
    game.reg_cmd("exit", Box::new(ExitCommand::default()));
    game.reg_cmd("debug", Box::new(debug::Command::default()))
}