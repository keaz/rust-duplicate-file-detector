

pub struct CmdArgs {
    pub root_folder: String,
    pub search_score: i64,
    pub use_sha: bool,
}

impl CmdArgs {
    fn from(root_folder: String, search_score: i64, use_sha: bool) -> Self {
        CmdArgs {
            root_folder,
            search_score,
            use_sha,
        }
    }
}


/// Retuns command line arguments as CmdArgs
///
/// let args = vec![String::from("-f=/Downloads/abc"),String::from("-s=65"),String::from("-sha=true")];
///
/// let cmd = extract_cmd(args).unwrap();
/// assert_eq!(String::from("/Downloads/abc"),cmd.root_folder);
/// assert_eq!(65,cmd.search_score);
/// assert_eq!(true,cmd.use_sha);
/// 
pub fn extract_cmd(args: Vec<String>) -> Result<CmdArgs, &'static str> {
    if args.len() == 0 {
        return Err("Not enough arguments, at least `-f` is required");
    }

    if args.len() > 3 {
        return Err("Too many arguments, expect only `-f`, `-s` and `-sha`");
    }

    let args: Vec<_> = args
        .iter()
        .map(|arg| arg.split("="))
        .map(|arg| arg.collect::<Vec<&str>>())
        .filter(|splited_args| splited_args.get(0).is_some() && splited_args.get(1).is_some())
        .collect();


    if args.iter().find(|splited_args|splited_args.contains(&"-f")).is_none() {
        return Err("Arguments does not contain -f");
    }

    let mut root_folder = "";
    let mut search_score = 90;
    let mut use_sha = false;

    for arg in args {
        if arg.len() > 2 {
            return Err("Invalid arguments");
        }

        let key = match arg.get(0) {
            None => return Err("Invalid arguments"),
            Some(value) => *value,
        };

        if key.starts_with("-f") {
            match arg.get(1) {
                None => return Err("Invalid argument for `-f`"),
                Some(value) => {
                    root_folder = value;
                }
            }
            continue;
        }

        if key.starts_with("-sha") {
            match arg.get(1) {
                None => return Err("Invalid argument for `-sha`"),
                Some(value) => {
                    use_sha = match value.parse::<bool>() {
                        Err(_) => return Err("Invalid argument for `-sha`"),
                        Ok(value) => value,
                    };
                }
            }
            continue;
        }

        if key.starts_with("-s") {
            match arg.get(1) {
                None => return Err("Invalid argument for `-s`"),
                Some(value) => {
                    search_score = match value.parse::<i64>() {
                        Err(_) => return Err("Invalid argument for `-s`"),
                        Ok(value) => value,
                    };
                }
            }
            continue;
        }
    }

    Ok(CmdArgs::from(
        root_folder.to_string(),
        search_score,
        use_sha,
    ))
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn extract_cmd_ok() {
        let args = vec![String::from("-f=/Downloads/abc"),String::from("-s=65"),String::from("-sha=true")];
        let cmd = extract_cmd(args).unwrap();
        assert_eq!(String::from("/Downloads/abc"),cmd.root_folder);
        assert_eq!(65,cmd.search_score);
        assert_eq!(true,cmd.use_sha);
    }

    #[test]
    fn extract_cmd_zero_args() {
        let args = vec![];
        let cmd = extract_cmd(args);
        match cmd {
            Err(msg) => assert_eq!("Not enough arguments, at least `-f` is required",msg),
            Ok(_) => println!("Will never happens")
        }
    }

    #[test]
    fn extract_cmd_more_args() {
        let args = vec![String::from("-f=/Downloads/abc"),String::from("-s=65"),String::from("-sha=true"),String::from("-xxx=true")];
        let cmd = extract_cmd(args);
        match cmd {
            Err(msg) => assert_eq!("Too many arguments, expect only `-f`, `-s` and `-sha`",msg),
            Ok(_) => println!("Will never happens")
        }
    }

    #[test]
    fn extract_cmd_no_root() {
        let args = vec![String::from("-fx=/Downloads/abc")];
        let cmd = extract_cmd(args);
        match cmd {
            Err(msg) => assert_eq!("Arguments does not contain -f",msg),
            Ok(_) => println!("Will never happens")
        }
    }
}
