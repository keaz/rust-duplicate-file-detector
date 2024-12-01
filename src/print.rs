use std::fmt::Display;

use tabled::Tabled;

#[derive(Tabled)]
pub struct Duplicate {
    pub file_name: String,
    pub duplicates: DisplayVec,
    pub size: String,
    pub count: usize,
}

impl Duplicate {
    pub fn from(file_name: String, duplicates: DisplayVec, size: String, count: usize) -> Self {
        Duplicate {
            file_name,
            duplicates,
            size,
            count,
        }
    }
}

pub struct DisplayVec(Vec<String>);

impl DisplayVec {
    pub fn new(duplicates: Vec<String>) -> Self {
        DisplayVec(duplicates)
    }
}

impl Display for DisplayVec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for duplicate in &self.0 {
            write!(f, "{}\n", duplicate)?;
        }
        Ok(())
    }
}
