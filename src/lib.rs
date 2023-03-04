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
    use std::ops::Index;
    use std::sync::Arc;
    use std::{ops::ControlFlow};
    use std::cmp::PartialEq;
    use itertools::Itertools;
    use rayon::prelude::*;
    use fuzzy_matcher::FuzzyMatcher;
    use fuzzy_matcher::skim::{SkimMatcherV2, SkimScoreConfig};
    use colored::Colorize;


    pub type ResultAsync<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;


    #[derive(Debug)]
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

    #[derive(PartialEq)]
    #[derive(Debug)]
    pub struct DuplicateKey{
        pub file_name: String,
        // pub sha: String,
        pub size: u64,
    }

    impl DuplicateKey {
        pub fn from(file_name: String, size: u64) -> Self{
            DuplicateKey { file_name, size }
        }
    }

    struct Duplicate{
        key: DuplicateKey,
        duplicates: Vec<FileData>,
    }

    impl Duplicate {
        fn from(key: DuplicateKey, duplicates: Vec<FileData>) -> Self {
            Duplicate {key,duplicates} 
        }
    }

    pub async fn search_duplicates(root_folder: &String) {
        let path = PathBuf::from(root_folder);
        let file_data: Vec<FileData> = vec![];
        let file_data_arch = Arc::new(Mutex::new(file_data));

        let reads = fs::read_dir(path).await;
        match reads {
            Err(_) => {
                return;
            },Ok(entries) => {
                task::block_on(walk_dir(entries,Arc::clone(&file_data_arch)));
            }
        }

        let mut file_data = (*file_data_arch).lock().await;
        let score_config = SkimScoreConfig {
            gap_extension: -1,
            ..Default::default()
        };
        
        println!("Collected {} files",file_data.len());
        println!("Started checking duplicates...");
        let matcher = SkimMatcherV2::default();
        let matcher = matcher.score_config(score_config);

        let mut total_size_of_duplicate = 0;

        let mut duplicate_files: Vec<Duplicate> = vec![];
        let mut count = 0;
        file_data.sort_by(|first,second| first.size.cmp(&second.size));
        loop  {
            if count == file_data.len() {
                break;
            }
            let a_file_date = file_data.get(count).unwrap();
            let duplicates: Vec<_> = file_data[count+1..file_data.len()].par_iter()
            .filter(|file| {
                let result = matcher.fuzzy_match(file.file_name.as_str(), a_file_date.file_name.as_str());
                match result {
                    None => false,
                    Some(score) => score >= 90 && a_file_date.size == file.size,
                }
            }).map(|file| file.clone()).collect();

            if !duplicates.is_empty() {

                println!("{} {} size {}","File " ,a_file_date.file_name.bold().green(),a_file_date.size.to_string().cyan());
                println!("{} ",a_file_date.path.red());
                duplicates.iter().for_each(|file|{
                    println!("{} ",file.path.red());
                    total_size_of_duplicate += file.size;
                });
                
                duplicate_files.push(Duplicate{key: DuplicateKey { file_name: a_file_date.file_name.clone(), size: a_file_date.size }, duplicates });
            }
    
            count +=1;
        }

        let size_ib_mbs = total_size_of_duplicate /(1024*1024);
        println!("{} {} MB","Total Size of duplicate files".bold().green(),size_ib_mbs.to_string().bold().green());

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
            let file_name =  path.file_name().ok_or("No filename").unwrap().to_str().unwrap();
            let size = metadata.len();
            let is_readonly = metadata.permissions().readonly();

            let the_file_data = FileData{ file_name: String::from(file_name), path: String::from(path.to_str().unwrap())  ,size ,last_modified,is_readonly, sha: String::from("") };
            let mut data = (*file_data).lock().await;
            data.push(the_file_data);
            // let sha = sha(&path).await;
            // match sha {
            //     None => {
            //         eprintln!("Failed to create sha for file {:?}",file_name);
            //     },
            //     Some(sha) => {
            //         let the_file_data = FileData{ file_name: String::from(file_name), path: String::from(path.to_str().unwrap())  ,size ,last_modified,is_readonly, sha };
            //         let mut data = (*file_data).lock().await;
            //         data.push(the_file_data);
            //     }
            // }
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

