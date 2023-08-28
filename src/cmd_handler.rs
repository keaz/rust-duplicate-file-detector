use clap::Parser;

/// CLI application that search duplicate files in a folder
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct CmdArgs {
    /// Root folder to search duplicate
    #[arg(short, long)]
    pub root_folder: String,

    /// Serch score for duplicate file names, default is 90
    #[arg(short, long, default_value_t = 90)]
    pub search_score: i64,

    /// Use sha256 to compare files, default is false
    #[arg(short, long, default_value_t = false)]
    pub use_sha: bool,
}

#[cfg(test)]
mod tests {}
