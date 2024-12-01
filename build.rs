use std::{
    fs::{self, File},
    io,
    path::Path,
};

use clap::{CommandFactory, Parser, ValueEnum};

fn main() {
    generate_completion().unwrap();
}

/// CLI application that search duplicate files in a folder
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct CmdArgs {
    /// Root folder to search duplicate
    #[arg(short, long)]
    pub root_folder: String,
}

#[derive(ValueEnum, Debug, Clone)]
pub enum Shell {
    Bash,
    Zsh,
}

pub fn generate_completion() -> Result<(), io::Error> {
    let mut cmd = CmdArgs::command();

    // Create the directory if it doesn't exist
    let bash_dir = ".bash_completion.d";
    let zsh_dir = ".zfunc";
    if !Path::new(bash_dir).exists() {
        fs::create_dir_all(bash_dir)?;
    }
    if !Path::new(zsh_dir).exists() {
        fs::create_dir_all(zsh_dir)?;
    }

    let mut bash_file = File::create(format!("{}/duplicate-checker.bash", bash_dir))?;
    let mut zsh_file = File::create(format!("{}/_duplicate-checker", zsh_dir))?;

    clap_complete::generate(
        clap_complete::shells::Bash,
        &mut cmd,
        "duplicate-checker",
        &mut bash_file,
    );
    clap_complete::generate(
        clap_complete::shells::Zsh,
        &mut cmd,
        "duplicate-checker",
        &mut zsh_file,
    );
    Ok(())
}
