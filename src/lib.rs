pub mod searcher {
    extern crate async_std;
    extern crate futures;
    
    use std::{fs::{self, Metadata}, path::{PathBuf}, ops::ControlFlow};
    use std::cmp::PartialEq;
    use itertools::Itertools;
    use sha256::try_digest;

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
        pub sha: String,
    }

    impl DuplicateKey {
        pub fn from(file_name: String, sha: String) -> Self{
            DuplicateKey { file_name, sha }
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

    pub fn search_duplicates(root_folder: &String, threads: u8)  {
        let path = PathBuf::from(root_folder);
        let mut file_data: Vec<FileData> = vec![];
        
        match fs::read_dir(path) {
            Err(er) => {
                eprintln!("Error reading directory {:?}",root_folder);
            },
            Ok(entries) => {
                walk_dir(entries, &mut file_data);
            }
        }

        file_data.iter()
        .group_by(|file| DuplicateKey::from(file.file_name.clone(), file.sha.clone()))
        .into_iter()
        .map(|(key, group)| Duplicate::from(key, group.into_iter().map(|file| file.clone()).collect_vec()))
        .filter(|duplicate| duplicate.duplicates.len() > 1)
        .for_each( |duplicate|
            {
                println!("Duplicate key {:?}",duplicate.key);
                duplicate.duplicates.iter().for_each(|file| 
                    {
                        println!("{:?}", file);
                    });
                println!("************")
            }
        );

    }

    fn walk_dir(entries: fs::ReadDir, file_data: &mut Vec<FileData>) {

        for entry in entries {
            match entry {
                Err(err) => {
                    eprintln!("Error reading entry {:?}",err);
                },
                Ok(dir_entry) => {
                    let path = dir_entry.path();
                    let metadata = folder_metadata(&path);

                    if let ControlFlow::Break(_) = visit_path(metadata, path, file_data) {
                        continue;
                    }

                }
            }
    
        }
    }

    fn visit_path(metadata: Option<Metadata>, path: PathBuf, file_data: &mut Vec<FileData>) -> ControlFlow<()> {
        match metadata {
            None => {
                eprintln!("Failed to read metadata of {:?}",path);
            },
            Some(metadata) => {
                let last_modified = match get_modified_date(&metadata) {
                    Ok(value) => value,
                    Err(value) => return value,
                };
                extract_detail_and_walk(last_modified, metadata, path, file_data );
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

    fn extract_detail_and_walk(last_modified: u64, metadata: Metadata, path: PathBuf, file_data: &mut Vec<FileData>) {
        if metadata.is_file() {
            let file_name =  path.file_name().ok_or("No filename").unwrap().to_str().unwrap();
            let size = metadata.len();
            let is_readonly = metadata.permissions().readonly();
            let sha = try_digest(path.as_path()).unwrap();
            let the_file_data = FileData{ file_name: String::from(file_name), path: String::from(path.to_str().unwrap())  ,size ,last_modified,is_readonly, sha };
            file_data.push(the_file_data);
            return;
        }

        if metadata.is_dir() {
            match path.read_dir() {
                Err(er) => {
                    eprintln!("Error reading directory {:?} error {:?}",path,er);
                },
                Ok(entries) => {
                    walk_dir(entries, file_data);
                }
            }
        }
    }

    fn folder_metadata(path: &PathBuf) -> Option<Metadata> {
        match fs::metadata(path) {
            Err(err) => {
                Option::None
            },
            Ok(metadata) =>{
                Option::Some(metadata)
            }
        } 
    }

}

