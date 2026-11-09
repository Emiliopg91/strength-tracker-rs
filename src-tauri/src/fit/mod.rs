pub mod parser;
pub mod writer;

/// A `.fit` file held in memory, as fetched from a device or from Connect.
#[derive(Debug, Clone)]
pub struct FitFile {
    /// File name, used to identify it in logs and as its name if it gets stored as pending.
    pub name: String,
    pub data: Vec<u8>,
    /// Connect activity the file was downloaded from, if any.
    pub activity_id: Option<u64>,
}

impl FitFile {
    pub fn new(name: impl Into<String>, data: Vec<u8>) -> Self {
        Self {
            name: name.into(),
            data,
            activity_id: None,
        }
    }

    /// Tags the file with the Connect activity it was downloaded from.
    pub fn with_activity_id(mut self, id: u64) -> Self {
        self.activity_id = Some(id);
        self
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
