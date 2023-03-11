pub mod searcher {
    extern crate async_std;
    extern crate futures;

    use async_std::fs::File;
    use async_std::sync::Mutex;
    use async_std::task;  
    use async_std::{fs::{self,Metadata},path::{PathBuf}};
    use async_std::io::BufReader;
    use data_encoding::HEXUPPER;
    use futures::{TryStreamExt,  AsyncReadExt};
    use ring::digest::{Digest, Context, SHA256};
    use std::fmt::format;
    use std::slice::SliceIndex;
    use std::sync::Arc;
    use std::ops::ControlFlow;
    use std::cmp::PartialEq;
    use core::cmp::Ord;
    use rayon::prelude::*;
    use fuzzy_matcher::FuzzyMatcher;
    use fuzzy_matcher::skim::{SkimMatcherV2, SkimScoreConfig};
    use colored::Colorize;
    use spinners::{Spinner, Spinners};

    pub type ResultAsync<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;


    #[derive(Debug)]
    #[derive(PartialEq,Eq, Ord)]
pub struct FileData {
        pub path: String,
        pub file_name: String,
        size: u64,
        last_modified: u64,
        is_readonly: bool,
        sha: String,
    }

    impl FileData {
        pub fn from(path: String, file_name: String,size: u64, last_modified: u64, is_readonly: bool, sha: String ) -> Self {
            FileData { path, file_name, size, last_modified, is_readonly, sha }
        }
    }

    impl Clone for FileData {
        fn clone(&self) -> Self {
            Self { path: self.path.clone(), file_name: self.file_name.clone(), size: self.size.clone(), 
                last_modified: self.last_modified.clone(), is_readonly: self.is_readonly.clone(), sha: self.sha.clone() }
        }
    }

    impl PartialOrd for FileData {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            if self.file_name.eq(&other.file_name) {
                return Some(self.size.cmp(&other.size));
            }

            Some(self.file_name.cmp(&other.file_name))
        }
    }

    #[derive(PartialEq)]
    #[derive(Debug)]
    pub struct DuplicateKey{
        pub file_name: String,
        pub size: u64,
    }

    impl DuplicateKey {
        pub fn from(file_name: String, size: u64) -> Self{
            DuplicateKey { file_name, size }
        }
    }

    pub async fn search_duplicates(root_folder: &String) {
        let msg =  format!("Looking in to path {:?}",root_folder);
        let mut sp = Spinner::new(Spinners::Aesthetic,msg.into());

        let path = PathBuf::from(root_folder);
        let file_data: Vec<FileData> = vec![];
        let file_data_arch = Arc::new(Mutex::new(file_data));

        let reads = fs::read_dir(path).await;
        match reads {
            Err(_) => {
                sp.stop();
                return;
            },Ok(entries) => {
                task::block_on(walk_dir(entries,Arc::clone(&file_data_arch)));
            }
        }
        sp.stop();
        
        let file_data = (*file_data_arch).lock().await;

        println!("Collected {} files",file_data.len());
        println!("Started checking duplicates...");

        let matcher = configure_matcher();

        let mut total_size_of_duplicate = 0;

        let mut count = 0;
        let mut file_data = file_data.clone();
        file_data.sort();

        loop  {
            if count == file_data.len() {
                break;
            }
            find_duplicate(&mut file_data, count, &matcher, &mut total_size_of_duplicate);
            count +=1;
        }

        let size_ib_mbs = total_size_of_duplicate /(1024*1024);
        println!("{} {} MB","Total Size of duplicate files".bold().green(),size_ib_mbs.to_string().bold().green());

    }

    fn find_duplicate(file_data: &mut Vec<FileData>, count: usize, matcher: &SkimMatcherV2, total_size_of_duplicate: &mut u64) {
        let a_file_date = file_data.get(count).unwrap();
        let sliced : Vec<FileData> =  file_data[count+1..file_data.len()].to_vec();
        let duplicates: Vec<_> = sliced.par_iter()
        .filter(|file| is_a_duplicate(matcher, file, a_file_date) ).map(|file| file.clone()).collect();

        if !duplicates.is_empty() {

            println!("{} {} size {}","File " ,a_file_date.file_name.bold().green(),a_file_date.size.to_string().cyan());
            println!("{} ",a_file_date.path.red());
            duplicates.iter().for_each(|file|{
                println!("{} ",file.path.red());
                *total_size_of_duplicate += file.size;
            });
    
            *file_data = file_data[duplicates.len() - 1..file_data.len()].to_vec();
        }
    }

    fn is_a_duplicate(matcher: &SkimMatcherV2, file: &&FileData, a_file_date: &FileData) -> bool {
        let result = matcher.fuzzy_match(file.file_name.as_str(), a_file_date.file_name.as_str());
        match result {
            None => false,
            Some(score) => score >= 90 && a_file_date.size == file.size,
        }
    }

    fn configure_matcher() -> SkimMatcherV2 {
        let score_config = SkimScoreConfig {
            gap_extension: -1,
            ..Default::default()
        };
        
        let matcher = SkimMatcherV2::default();
        matcher.score_config(score_config)
    }

    async fn walk_dir(mut entries: fs::ReadDir, file_data: Arc<Mutex<Vec<FileData>>>) {
        
        while let Ok(Some(dir_entry)) = entries.try_next().await {
            let path = dir_entry.path();
            let metadata = folder_metadata(&path).await;

            if let ControlFlow::Break(_) = visit_path(metadata, path, file_data.clone()).await {
                continue;
            }
        }

    }

    async fn visit_path(metadata: Option<Metadata>, path: PathBuf, file_data: Arc<Mutex<Vec<FileData>>>) -> ControlFlow<()> {
        match metadata {
            None => {
                eprintln!("Failed to read metadata of {:?}",path);
            },
            Some(metadata) => {
                let last_modified = match get_modified_date(&metadata) {
                    Ok(value) => value,
                    Err(value) => return value,
                };
                extract_detail_and_walk(last_modified, metadata, path, file_data.clone() ).await;
            }
        }
        ControlFlow::Continue(())
    }

    fn get_modified_date(metadata: &Metadata) -> Result<u64, ControlFlow<()>> {
        let last_modified = match metadata.modified() {
            Err(err) => {
                eprintln!("Error reading modified value from {:?}",err);
                return Err(ControlFlow::Break(()));
            },
            Ok(sys_time) => {
                match sys_time.elapsed() {
                    Err(err) => {
                        eprintln!("Error reading elapsed value from {:?}",err);
                        return Err(ControlFlow::Break(()));
                    },
                    Ok(time) => {
                        time.as_secs()
                    }
                } 
            }
        };
        Ok(last_modified)
    }

    async fn extract_detail_and_walk(last_modified: u64, metadata: Metadata, path: PathBuf, file_data: Arc<Mutex<Vec<FileData>>>) {
        if metadata.is_file() {
            let the_file_data = extract_file_data(&path, &metadata, last_modified);
            let mut data = (*file_data).lock().await;
            data.push(the_file_data);
            return;
            
        }

        if metadata.is_dir() {
            match path.read_dir().await {
                Err(er) => {
                    eprintln!("Error reading directory {:?} error {:?}",path,er);
                },
                Ok(entries) => {
                    task::block_on( walk_dir(entries, file_data.clone()));
                }
            }
        }
    }

    fn extract_file_data(path: &PathBuf, metadata: &Metadata, last_modified: u64) -> FileData {
        let file_name =  path.file_name().ok_or("No filename").unwrap().to_str().unwrap();
        let size = metadata.len();
        let is_readonly = metadata.permissions().readonly();

        let the_file_data = FileData{ file_name: String::from(file_name), path: String::from(path.to_str().unwrap())  ,size ,last_modified,is_readonly, sha: String::from("") };
        the_file_data
    }

    async fn folder_metadata(path: &PathBuf) -> Option<Metadata> {
        match fs::metadata(path).await {
            Err(_err) => {
                Option::None
            },
            Ok(metadata) =>{
                Option::Some(metadata)
            }
        } 
    }


    async fn sha256_digest(mut reader: BufReader<File>) -> Option<Digest> {
        let mut context = Context::new(&SHA256);
        let mut buffer = [0; 1024];
    
        loop {
            let count = reader.read(&mut buffer).await.unwrap();
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
    

}

