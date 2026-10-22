pub mod errors;
pub mod workout;

use std::{fs::File, io::BufWriter, path::Path};

use embedded_io_adapters::std::FromStd;
use rustyfit::{
    Encoder,
    profile::{
        mesgdef::{FileCreator, FileId},
        typedef::{self, DateTime, Manufacturer},
    },
    proto::{FIT, Message},
};

use crate::{
    dao::settings::WeightUnit,
    utils::{constants, translations::Languages},
};

use self::errors::WriteFitFileError;

pub trait ToFitMessages {
    fn file_type(&self) -> typedef::File;
    fn to_fit_messages(
        &self,
        lang: Languages,
        weight_unit: WeightUnit,
    ) -> errors::Result<Vec<Message>>;
}

pub struct FitWriter<'a, T>
where
    T: ToFitMessages,
{
    obj: &'a T,
}

impl<'a, T> FitWriter<'a, T>
where
    T: ToFitMessages,
{
    pub fn from(obj: &'a T) -> Self {
        Self { obj }
    }

    fn file_id(&self) -> Message {
        let mut id = FileId::new();
        id.r#type = self.obj.file_type();
        id.manufacturer = Manufacturer::DEVELOPMENT;
        id.product_name = constants::APP_NAME.clone().into();
        id.time_created = DateTime::from_unix_timestamp(chrono::Utc::now().timestamp());
        id.into()
    }

    fn file_creator() -> Message {
        let major = constants::APP_SEM_VERSION.major as u16;
        let minor = constants::APP_SEM_VERSION.minor as u16;
        let mut creator = FileCreator::new();
        creator.software_version = major * 100 + minor;
        creator.into()
    }

    /// Encodes `item` as a complete FIT file, consuming the writer.
    pub fn write<P>(self, path: P, lang: Languages, weight_unit: WeightUnit) -> errors::Result<()>
    where
        P: AsRef<Path>,
    {
        let mut messages = vec![self.file_id(), Self::file_creator()];
        messages.extend(self.obj.to_fit_messages(lang, weight_unit)?);

        let mut fit = FIT {
            messages,
            ..Default::default()
        };

        let file = File::create(path.as_ref())
            .map_err(|e| WriteFitFileError::FileCreating(path.as_ref().display().to_string(), e))?;
        let mut writer = FromStd::new(BufWriter::new(file));
        let mut encoder = Encoder::new();
        encoder.encode(&mut writer, &mut fit).map_err(|e| {
            WriteFitFileError::FileWriting(
                path.as_ref().display().to_string(),
                format!("{e:?}").into(),
            )
        })?;

        Ok(())
    }
}
