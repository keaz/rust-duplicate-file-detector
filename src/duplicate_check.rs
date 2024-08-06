extern crate futures;

use colored::Colorize;
use core::cmp::Ord;
use data_encoding::HEXUPPER;
use futures::future::BoxFuture;
use futures::{AsyncReadExt, FutureExt};
use fuzzy_matcher::skim::{SkimMatcherV2, SkimScoreConfig};
use fuzzy_matcher::FuzzyMatcher;
use rayon::prelude::*;
use ring::digest::{Context, Digest, SHA256};
use spinners::{Spinner, Spinners};
use std::ops::ControlFlow;
use std::sync::Arc;
use std::{cmp::PartialEq, fs::Metadata, path::PathBuf};
use tabled::Table;
use tokio::fs::{self};
use tokio::fs::{File, ReadDir};
use tokio::io::BufReader;
use tokio::sync::Mutex;

use crate::cmd_handler::CmdArgs;
use crate::print::Duplicate;

#[derive(Debug, PartialEq, Eq)]
pub struct FileData {
    pub path: String,
    pub file_name: String,
    size: u64,
    last_modified: u64,
    is_readonly: bool,
    sha: String,
}

impl FileData {
    pub fn from(
        path: String,
        file_name: String,
        size: u64,
        last_modified: u64,
        is_readonly: bool,
        sha: String,
    ) -> Self {
        FileData {
            path,
            file_name,
            size,
            last_modified,
            is_readonly,
            sha,
        }
    }
}

impl Clone for FileData {
    fn clone(&self) -> Self {
        Self {
            path: self.path.clone(),
            file_name: self.file_name.clone(),
            size: self.size,
            last_modified: self.last_modified,
            is_readonly: self.is_readonly,
            sha: self.sha.clone(),
        }
    }
}

impl PartialOrd for FileData {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FileData {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.file_name.eq(&other.file_name) {
            return self.size.cmp(&other.size);
        }

        self.file_name.cmp(&other.file_name)
    }
}

#[derive(PartialEq, Debug)]
pub struct DuplicateKey {
    pub file_name: String,
    pub size: u64,
}

impl DuplicateKey {
    pub fn from(file_name: String, size: u64) -> Self {
        DuplicateKey { file_name, size }
    }
}

pub async fn search_duplicates(cmds: &CmdArgs) {
    let msg = format!("Looking in to path {:?}", cmds.root_folder);
    let mut sp = Spinner::new(Spinners::Aesthetic, msg);

    let path = PathBuf::from(cmds.root_folder.clone());
    let file_data: Vec<FileData> = vec![];
    let file_data_arch = Arc::new(Mutex::new(file_data));

    let reads = fs::read_dir(path).await;
    match reads {
        Err(_) => {
            sp.stop();
            return;
        }
        Ok(entries) => {
            let file_data_arch = Arc::clone(&file_data_arch);
            walk_dir(entries, file_data_arch).await;
        }
    }
    sp.stop();

    let file_data = (*file_data_arch).lock().await;

    println!("Collected {} files", file_data.len());
    println!(
        "Started checking duplicates using score {}...",
        cmds.search_score
    );

    let matcher = configure_matcher();

    let mut total_size_of_duplicate = 0;

    let mut count = 0;
    let mut file_data = file_data.clone();
    file_data.sort();

    let mut duplicate_map = vec![];
    loop {
        if count == file_data.len() {
            break;
        }

        let duplicate = find_duplicate(
            &mut file_data,
            count,
            &matcher,
            &mut total_size_of_duplicate,
            cmds.search_score,
        );
        duplicate_map.push(duplicate);
        count += 1;
    }

    let mut duplicate = vec![];
    for (file, duplicates) in duplicate_map {
        let mb = 1024 * 1024;

        let size = if file.size > mb {
            format!("{} KB", (file.size / 1024 * 1024))
        } else if file.size > 1024 {
            format!("{} MB", file.size / 1024)
        } else {
            format!("{} Byte", file.size)
        };
        duplicate.push(Duplicate::from(
            file.file_name,
            file.sha,
            size,
            duplicates.len(),
        ));
    }

    let table = Table::new(duplicate).to_string();
    tokio::fs::write("duplicate.txt", table.as_bytes())
        .await
        .unwrap();
    println!("{}", table);
}

fn find_duplicate(
    file_data: &mut Vec<FileData>,
    count: usize,
    matcher: &SkimMatcherV2,
    total_size_of_duplicate: &mut u64,
    score_value: i64,
) -> (FileData, Vec<FileData>) {
    let a_file_date = file_data.get(count).unwrap();
    let sliced: Vec<FileData> = file_data[count + 1..file_data.len()].to_vec();
    let duplicates: Vec<_> = sliced
        .par_iter()
        .filter(|file| is_a_duplicate(matcher, file, a_file_date, score_value))
        .map(|file| file.clone())
        .collect();

    let file = a_file_date.clone();
    if !duplicates.is_empty() {
        duplicates.iter().for_each(|file| {
            *total_size_of_duplicate += file.size;
        });

        *file_data = file_data[duplicates.len() - 1..file_data.len()].to_vec();
    }

    (file, duplicates)
}

fn is_a_duplicate(
    matcher: &SkimMatcherV2,
    file: &&FileData,
    a_file_date: &FileData,
    score_value: i64,
) -> bool {
    let result = matcher.fuzzy_match(file.file_name.as_str(), a_file_date.file_name.as_str());
    match result {
        None => false,
        Some(score) => score >= score_value && a_file_date.sha.eq_ignore_ascii_case(&file.sha),
    }
}

fn configure_matcher() -> SkimMatcherV2 {
    let score_config = SkimScoreConfig {
        ..Default::default()
    };

    let matcher = SkimMatcherV2::default();
    matcher.score_config(score_config)
}

async fn walk_dir(mut entries: fs::ReadDir, file_data: Arc<Mutex<Vec<FileData>>>) {
    while let Ok(Some(dir_entry)) = entries.next_entry().await {
        let path = dir_entry.path();
        let metadata = folder_metadata(&path).await;

        if let ControlFlow::Break(_) = visit_path(metadata, path, file_data.clone()).await {
            continue;
        }
    }
}

async fn visit_path(
    metadata: Option<Metadata>,
    path: PathBuf,
    file_data: Arc<Mutex<Vec<FileData>>>,
) -> ControlFlow<()> {
    match metadata {
        None => {
            eprintln!("Failed to read metadata of {:?}", path);
        }
        Some(metadata) => {
            let last_modified = match get_modified_date(&metadata) {
                Ok(value) => value,
                Err(value) => return value,
            };
            extract_detail_and_walk(last_modified, metadata, path, file_data.clone()).await;
        }
    }
    ControlFlow::Continue(())
}

fn get_modified_date(metadata: &Metadata) -> Result<u64, ControlFlow<()>> {
    let last_modified = match metadata.modified() {
        Err(err) => {
            eprintln!("Error reading modified value from {:?}", err);
            return Err(ControlFlow::Break(()));
        }
        Ok(sys_time) => match sys_time.elapsed() {
            Err(err) => {
                eprintln!("Error reading elapsed value from {:?}", err);
                return Err(ControlFlow::Break(()));
            }
            Ok(time) => time.as_secs(),
        },
    };
    Ok(last_modified)
}

fn extract_detail_and_walk(
    last_modified: u64,
    metadata: Metadata,
    path: PathBuf,
    file_data: Arc<Mutex<Vec<FileData>>>,
) -> BoxFuture<'static, ()> {
    async move {
        if metadata.is_file() {
            let the_file_data = extract_file_data(&path, &metadata, last_modified).await;
            let mut data = (*file_data).lock().await;
            data.push(the_file_data);
            return;
        }

        if metadata.is_dir() {
            match fs::read_dir(&path).await {
                Err(er) => {
                    eprintln!("Error reading directory {:?} error {:?}", path, er);
                }
                Ok(entries) => {
                    walk(entries, file_data).await;
                }
            }
        }
    }
    .boxed()
}

async fn walk(entries: ReadDir, file_data: Arc<Mutex<Vec<FileData>>>) {
    let file_data = file_data.clone();
    walk_dir(entries, file_data).await;
}

async fn extract_file_data(path: &PathBuf, metadata: &Metadata, last_modified: u64) -> FileData {
    let file_name = path
        .file_name()
        .ok_or("No filename")
        .unwrap()
        .to_str()
        .unwrap();
    let size = metadata.len();
    let is_readonly = metadata.permissions().readonly();
    let sha = sha(path).await.unwrap();

    let the_file_data = FileData {
        file_name: String::from(file_name),
        path: String::from(path.to_str().unwrap()),
        size,
        last_modified,
        is_readonly,
        sha,
    };
    the_file_data
}

async fn folder_metadata(path: &PathBuf) -> Option<Metadata> {
    match fs::metadata(path).await {
        Err(err) => {
            eprintln!("Error reading metadata {:?} error {:?}", path, err);
            Option::None
        }
        Ok(metadata) => Option::Some(metadata),
    }
}

async fn sha256_digest(reader: BufReader<File>) -> Option<Digest> {
    let mut context = Context::new(&SHA256);
    let mut buffer = [0; 1024];

    loop {
        let count = reader.buffer().read(&mut buffer).await.unwrap();
        if count == 0 {
            break;
        }
        context.update(&buffer[..count]);
    }

    Some(context.finish())
}

async fn sha(path: &PathBuf) -> Option<String> {
    let input = File::open(path).await.unwrap();
    let reader = BufReader::new(input);
    let digest = sha256_digest(reader).await?;

    std::option::Option::Some(HEXUPPER.encode(digest.as_ref()))
}
