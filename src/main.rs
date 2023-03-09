use std::env;
use async_std::task;
use duplicate_checker::searcher::search_duplicates;

fn main() {
    let args: Vec<String> = env::args().collect();
    let file_path = &args[1];
    let search_future = search_duplicates(file_path);
    task::block_on(search_future);
}
