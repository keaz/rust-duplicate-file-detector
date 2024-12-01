extern crate futures;

use core::cmp::Ord;
use data_encoding::HEXUPPER;
use futures::future::BoxFuture;
use futures::{AsyncReadExt, FutureExt};
use rayon::prelude::*;
use ring::digest::{Context, Digest, SHA256};
use spinners::{Spinner, Spinners};
use std::sync::Arc;
use std::{cmp::PartialEq, fs::Metadata, path::PathBuf};
use tabled::Table;
use tokio::fs::{self};
use tokio::fs::{File, ReadDir};
use tokio::io::BufReader;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::cmd_handler::CmdArgs;
use crate::print::{DisplayVec, Duplicate};

#[derive(Debug, PartialEq, Eq)]
pub struct FileData {
    pub path: String,
    pub file_name: String,
    size: u64,
    last_modified: u64,
    is_readonly: bool,
}

impl Clone for FileData {
    fn clone(&self) -> Self {
        Self {
            path: self.path.clone(),
            file_name: self.file_name.clone(),
            size: self.size,
            last_modified: self.last_modified,
            is_readonly: self.is_readonly,
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

    println!("\nCollected {} files", file_data.len());

    let mut total_size_of_duplicate = 0;

    let mut count = 0;
    let mut file_data = file_data.clone();
    file_data.sort();

    let mut duplicate_map = vec![];
    loop {
        if count == file_data.len() {
            break;
        }

        let duplicate = find_duplicate(&mut file_data, count, &mut total_size_of_duplicate).await;
        duplicate_map.push(duplicate);
        count += 1;
    }

    let mut duplicate_map: Vec<&(FileData, Vec<FileData>)> =
        duplicate_map.iter().filter(|f| f.1.len() > 0).collect();

    duplicate_map.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

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
        let duplicate_vec = duplicates
            .iter()
            .map(|file| file.path.clone())
            .collect::<Vec<String>>();

        duplicate.push(Duplicate::from(
            file.path.clone(),
            DisplayVec::new(duplicate_vec),
            size,
            duplicates.len(),
        ));
    }

    let table = Table::new(duplicate).to_string();
    tokio::fs::write("duplicate.txt", table.as_bytes())
        .await
        .unwrap();
}

async fn find_duplicate(
    file_data: &mut Vec<FileData>,
    count: usize,
    total_size_of_duplicate: &mut u64,
) -> (FileData, Vec<FileData>) {
    let a_file_date = file_data.get(count).unwrap();
    let sliced: Vec<FileData> = file_data[count + 1..file_data.len()].to_vec();

    // Collect the results of is_a_duplicate asynchronously
    let duplicate_checks: Vec<_> =
        futures::future::join_all(sliced.iter().map(|file| is_a_duplicate(file, a_file_date)))
            .await;

    let duplicates: Vec<_> = sliced
        .into_par_iter()
        .zip(duplicate_checks.into_par_iter())
        .filter_map(
            |(file, is_duplicate)| {
                if is_duplicate {
                    Some(file)
                } else {
                    None
                }
            },
        )
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

async fn is_a_duplicate(file: &FileData, a_file_date: &FileData) -> bool {
    if file.size != a_file_date.size {
        return false;
    }

    let file = PathBuf::from(&file.path);
    let a_file_date = PathBuf::from(&a_file_date.path);

    let file_sha = sha(&file).await.unwrap();
    let a_file_date_sha = sha(&a_file_date).await.unwrap();

    file_sha.eq(&a_file_date_sha)
}

async fn walk_dir(mut entries: fs::ReadDir, file_data: Arc<Mutex<Vec<FileData>>>) {
    let mut tasks = vec![];
    while let Ok(Some(dir_entry)) = entries.next_entry().await {
        let path = dir_entry.path();
        let metadata = folder_metadata(&path).await;

        if let Some(handler) = visit_path(metadata, path, file_data.clone()).await {
            tasks.push(handler);
        }
    }

    for task in tasks {
        task.await.unwrap();
    }
}

async fn visit_path(
    metadata: Option<Metadata>,
    path: PathBuf,
    file_data: Arc<Mutex<Vec<FileData>>>,
) -> Option<JoinHandle<()>> {
    match metadata {
        None => {
            eprintln!("Failed to read metadata of {:?}", path);
        }
        Some(metadata) => {
            let last_modified = match get_modified_date(&metadata) {
                Ok(value) => value,
                Err(_) => return None,
            };
            let task =
                extract_detail_and_walk(last_modified, metadata, path, file_data.clone()).await;
            return task;
        }
    }
    None
}

fn get_modified_date(metadata: &Metadata) -> Result<u64, String> {
    let last_modified = match metadata.modified() {
        Err(err) => {
            eprintln!("Error reading modified value from {:?}", err);
            return Err(format!("Error reading modified value from {:?}", err));
        }
        Ok(sys_time) => match sys_time.elapsed() {
            Err(err) => {
                eprintln!("Error reading elapsed value from {:?}", err);
                return Err(format!("Error reading elapsed value from {:?}", err));
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
) -> BoxFuture<'static, Option<JoinHandle<()>>> {
    async move {
        if metadata.is_file() {
            let the_file_data = extract_file_data(&path, &metadata, last_modified).await;
            let mut data = (*file_data).lock().await;
            data.push(the_file_data);
            return None;
        }

        if metadata.is_dir() {
            let handler = tokio::spawn(async move {
                match fs::read_dir(&path).await {
                    Err(er) => {
                        eprintln!("Error reading directory {:?} error {:?}", path, er);
                    }
                    Ok(entries) => {
                        walk(entries, file_data).await;
                    }
                }
            });
            return Some(handler);
        }
        None
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

    let the_file_data = FileData {
        file_name: String::from(file_name),
        path: String::from(path.to_str().unwrap()),
        size,
        last_modified,
        is_readonly,
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
