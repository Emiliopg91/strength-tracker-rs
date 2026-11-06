pub mod session;

use std::io::Cursor;
#[cfg(any(test, debug_assertions))]
use std::path::Path;

use embedded_io_adapters::std::FromStd;
use ouroboros::self_referencing;
use rustyfit::{Decoder, StreamDecoder};

#[cfg(any(test, debug_assertions))]
use self::errors::ParseFitFileError;
use crate::fit::FitFile;

pub mod errors;
pub mod workout;

#[self_referencing]
pub struct FitParser {
    name: String,
    decoder: Decoder,

    #[borrows(mut decoder)]
    #[not_covariant]
    stream: StreamDecoder<'this, FromStd<Cursor<Vec<u8>>>>,
}

impl FitParser {
    /// Reads and parses the file at `path`. Only debug tooling and tests read from disk; synced
    /// files are parsed from memory with [`FitParser::from_fit_file`].
    #[cfg(any(test, debug_assertions))]
    pub fn from_file<P>(path: P) -> errors::Result<Self>
    where
        P: AsRef<Path>,
    {
        let path_ref = path.as_ref();
        let file = FitFile::read(path_ref)
            .map_err(|e| ParseFitFileError::FileOpening(path_ref.display().to_string(), e))?;
        Ok(Self::from_fit_file(file))
    }

    /// Parses a file already held in memory.
    pub fn from_fit_file(file: FitFile) -> Self {
        let reader = FromStd::new(Cursor::new(file.data));

        FitParserBuilder {
            name: file.name,
            decoder: Decoder::new(),
            stream_builder: |decoder| decoder.stream(reader),
        }
        .build()
    }

    #[cfg(debug_assertions)]
    pub fn debug_dump(mut self) -> Result<(), Box<dyn std::error::Error>> {
        use rustyfit::{DecoderEvent, StreamingIterator};

        let path_string = self.borrow_name().clone();
        let dump_path = format!("{path_string}.json");

        let entries = self.with_stream_mut(|stream| -> errors::Result<_> {
            let mut entries = Vec::new();
            while let Some(event) = stream.next() {
                let event = event.map_err(|e| {
                    ParseFitFileError::FileReading(path_string.clone(), Box::new(e))
                })?;

                if let DecoderEvent::Message(mesg) = event {
                    entries.push(mesg.clone());
                }
            }
            Ok(entries)
        })?;

        let json = serde_json::to_string_pretty(&entries)?;
        std::fs::write(&dump_path, json)?;
        Ok(())
    }
}

/// Inverse of [`FitParser::debug_dump`]: encodes a JSON dump back into a FIT file, written next to it
/// without the `.json` extension (or with `.fit` appended). Never overwrites an existing file.
#[cfg(debug_assertions)]
pub fn debug_encode<P>(path: P) -> Result<std::path::PathBuf, Box<dyn std::error::Error>>
where
    P: AsRef<Path>,
{
    use std::{fs::File, path::PathBuf};

    use rustyfit::{
        Encoder,
        proto::{FIT, Message, Value},
    };

    // serde_json dumps NaN (the FIT invalid float) as null, which does not deserialize back
    const NAN_SENTINEL: f32 = 123456.0;

    let path = path.as_ref();
    let fit_path = match path.extension() {
        Some(ext) if ext == "json" => path.with_extension(""),
        _ => PathBuf::from(format!("{}.fit", path.display())),
    };
    if fit_path.exists() {
        return Err(format!("{} already exists", fit_path.display()).into());
    }

    let json = std::fs::read_to_string(path)?.replace("\"c\": null", "\"c\": 123456.0");
    let mut messages: Vec<Message> = serde_json::from_str(&json)?;
    for field in messages.iter_mut().flat_map(|m| m.fields.iter_mut()) {
        if let Value::Float32(v) = &mut field.value
            && *v == NAN_SENTINEL
        {
            *v = f32::from_bits(u32::MAX);
        }
    }

    let mut fit = FIT {
        messages,
        ..Default::default()
    };
    let writer = FromStd::new(std::io::BufWriter::new(File::create_new(&fit_path)?));
    Encoder::new()
        .encode(writer, &mut fit)
        .map_err(|e| format!("{e:?}"))?;

    Ok(fit_path)
}
