use std::{env, process};
use async_std::task;
use clap::Parser;
use duplicate_checker::{searcher::search_duplicates, cmd_handler::CmdArgs};

fn main() {
    let cmds = CmdArgs::parse_from(env::args_os());
    let search_future = search_duplicates(&cmds);
    task::block_on(search_future);
    
}
