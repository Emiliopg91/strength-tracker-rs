pub mod parser;
pub mod writer;

/// A `.fit` file held in memory, as fetched from a device or from Connect.
#[derive(Debug, Clone)]
pub struct FitFile {
    /// File name, used to identify it in logs and as its name if it gets stored as pending.
    pub name: String,
    pub data: Vec<u8>,
}

impl FitFile {
    pub fn new(name: impl Into<String>, data: Vec<u8>) -> Self {
        Self {
            name: name.into(),
            data,
        }
    }

    /// Reads the file at `path` into memory, named after its full path.
    pub fn read<P>(path: P) -> std::io::Result<Self>
    where
        P: AsRef<std::path::Path>,
    {
        let path = path.as_ref();
        Ok(Self::new(path.display().to_string(), std::fs::read(path)?))
    }
}
