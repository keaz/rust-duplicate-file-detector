use clap::Parser;
use duplicate_checker::{cmd_handler::CmdArgs, searcher::search_duplicates};
use std::env;

#[tokio::main]
async fn main() {
    let cmds = CmdArgs::parse_from(env::args_os());
    search_duplicates(&cmds).await;
}
