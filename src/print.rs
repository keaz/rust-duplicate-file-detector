use tabled::{Table, Tabled};

#[derive(Tabled)]
pub struct Duplicate {
    pub file_name: String,
    pub sha: String,
    pub size: String,
    pub count: usize,
}

impl Duplicate {
    pub fn from(file_name: String, sha: String, size: String, count: usize) -> Self {
        Duplicate {
            file_name,
            sha,
            size,
            count,
        }
    }
}
