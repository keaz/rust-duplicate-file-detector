use std::env;
use duplicate_checker::searcher::search_duplicates;

fn main() {
    let args: Vec<String> = env::args().collect();
    let file_path = &args[1];
    println!("Looking in to path {:?}",file_path);
    search_duplicates(file_path, 1);
}
