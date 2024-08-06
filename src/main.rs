use clap::Parser;
use std::env;

use self::cmd_handler::CmdArgs;
use duplicate_check::search_duplicates;

mod cmd_handler;
mod duplicate_check;
mod print;

#[tokio::main]
async fn main() {
    let cmds = CmdArgs::parse_from(env::args_os());
    search_duplicates(&cmds).await;
}
