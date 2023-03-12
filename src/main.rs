use std::{env, process};
use async_std::task;
use duplicate_checker::searcher::search_duplicates;
use duplicate_checker::cmd_handler::extract_cmd;

fn main() {
    let args: Vec<String> = env::args().collect();
    let cmds = extract_cmd(args);
    match cmds {
        Err(msg) => {
            eprintln!("{}", &msg);
            process::exit(0);
        },
        Ok(value) => {
            
            let search_future = search_duplicates(&value);
            task::block_on(search_future);
        }
    }
    
}
