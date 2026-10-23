use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
    time::Instant,
};

use bytes::Bytes;
use mtp_rs::{MtpDevice, NewObjectInfo, ObjectHandle, ObjectInfo, Storage};
use tauri_plugin_log::log::{debug, error, info};
use tokio::{fs, sync::Mutex};

use crate::{
    dto::devices::DeviceListItem,
    mtp::errors::{MtpError, Result},
    utils::constants,
};
#[cfg(debug_assertions)]
pub mod dump;
pub mod errors;

pub static MTP_CLIENT_INST: LazyLock<Mutex<MtpClient>> = LazyLock::new(|| Mutex::new(MtpClient {}));

pub struct MtpClient {}

/// True for regular files with a `.fit` extension (case-insensitive); skips folders and anything else.
fn is_fit_file(obj: &ObjectInfo) -> bool {
    obj.is_file()
        && obj
            .filename
            .rsplit_once('.')
            .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("fit"))
}

/// Finds the connected device with serial number `serial` and opens it. The caller must `close()` it.
async fn open_device(serial: &str) -> Result<MtpDevice> {
    let devices_info = MtpDevice::list_devices().map_err(MtpError::ListDevices)?;
    let device_info = devices_info
        .iter()
        .find(|d| {
            d.serial_number
                .as_ref()
                .is_some_and(|serial_n| serial_n == serial)
        })
        .ok_or_else(|| MtpError::MissingDevice(serial.to_string()))?;

    let device = MtpDevice::open_by_location(device_info.location_id)
        .await
        .map_err(|e| MtpError::OpenDevice(device_info.location_id, e))?;
    info!(
        "Found device {} {} with S/N {}",
        device.device_info().manufacturer,
        device.device_info().model,
        serial
    );

    Ok(device)
}

/// Looks up the `GARMIN/<folder>` folder on `storage`.
async fn find_garmin_folder(storage: &Storage, serial: &str, folder: &str) -> Result<ObjectInfo> {
    debug!("Entering into GARMIN folder...");
    let garmin_folder =
        find_child(storage, None, constants::MTP_GARMIN_ROOT_FOLDER, serial).await?;

    debug!("Entering into GARMIN/{} folder...", folder);
    find_child(storage, Some(garmin_folder.handle), folder, serial).await
}

/// Lists the objects inside `GARMIN/<folder>` on `storage`.
async fn list_garmin_folder(
    storage: &Storage,
    serial: &str,
    folder: &str,
) -> Result<Vec<ObjectInfo>> {
    let target = find_garmin_folder(storage, serial, folder).await?;

    storage
        .list_objects(Some(target.handle))
        .await
        .map_err(MtpError::ListFiles)
}

/// Looks up the object named `name` directly under `parent` (`None` is the storage root).
async fn find_child(
    storage: &Storage,
    parent: Option<ObjectHandle>,
    name: &str,
    serial: &str,
) -> Result<ObjectInfo> {
    storage
        .list_objects(parent)
        .await
        .map_err(MtpError::ListFiles)?
        .into_iter()
        .find(|oi| oi.filename == name)
        .ok_or_else(|| MtpError::NoStorageDevice(serial.to_string()))
}

/// Downloads `objs` into `dst_dir` (created if missing) and returns the local paths of those that
/// succeeded. A file that fails to download is logged and skipped; a failed write is an error.
async fn download_files(
    storage: &Storage,
    objs: Vec<ObjectInfo>,
    dst_dir: &Path,
) -> Result<Vec<PathBuf>> {
    fs::create_dir_all(dst_dir)
        .await
        .map_err(|e| MtpError::ErrorCreatingDownloadFolder(dst_dir.display().to_string(), e))?;

    info!("Downloading files...");
    let t0 = Instant::now();
    let mut size = 0_f64;
    let mut paths = Vec::new();
    for obj in objs {
        match storage.download_to_vec(obj.handle).await {
            Ok(bytes) => {
                let path = dst_dir.join(&obj.filename);
                fs::write(&path, &bytes)
                    .await
                    .map_err(|e| MtpError::WriteData(path.display().to_string(), e))?;
                size += bytes.len() as f64;
                paths.push(path);
            }
            Err(e) => {
                error!("Error downloading file {}: {}", obj.filename, e)
            }
        }
    }

    let elapsed_secs = t0.elapsed().as_secs_f64();
    info!(
        "{} files downloaded in {:.3}s ({:.2} MB/s)",
        paths.len(),
        elapsed_secs,
        (size / (1024 * 1024) as f64) / elapsed_secs
    );

    Ok(paths)
}

/// Deletes a file from MTP storage.
async fn delete_file(storage: &Storage, obj: ObjectInfo) -> Result<()> {
    info!("Deleting file {}...", obj.filename);
    let t0 = Instant::now();
    match storage.delete(obj.handle).await {
        Ok(()) => {
            let elapsed_secs = t0.elapsed().as_secs_f64();
            info!("File deleted in {:.3}s", elapsed_secs,);
            Ok(())
        }
        Err(e) => {
            error!("Error deleting file {}: {}", obj.filename, e);
            Err(MtpError::DeleteFile(obj.filename, e))
        }
    }
}

/// Uploads the local file `src` into the folder `parent` on `storage` and returns the new handle.
/// MTP cannot overwrite, so any file with the same name in `parent` is deleted first.
async fn upload_file(storage: &Storage, parent: ObjectHandle, src: &Path) -> Result<ObjectHandle> {
    let filename = src
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| MtpError::InvalidFileName(src.display().to_string()))?;
    let data = fs::read(src)
        .await
        .map_err(|e| MtpError::ReadData(src.display().to_string(), e))?;

    let existing = storage
        .list_objects(Some(parent))
        .await
        .map_err(MtpError::ListFiles)?;
    for obj in existing
        .into_iter()
        .filter(|o| o.is_file() && o.filename == filename)
    {
        info!("Replacing existing file {}", filename);
        storage
            .delete(obj.handle)
            .await
            .map_err(|e| MtpError::DeleteFile(filename.clone(), e))?;
    }

    info!("Uploading file {} ({} bytes)...", filename, data.len());
    let t0 = Instant::now();
    let new_info = NewObjectInfo::file(filename.clone(), data.len() as u64);
    let stream = tokio_stream::iter([Ok(Bytes::from(data))]);
    match storage.upload(Some(parent), new_info, stream).await {
        Ok(handle) => {
            info!(
                "File {} uploaded in {:.3}s",
                filename,
                t0.elapsed().as_secs_f64()
            );
            Ok(handle)
        }
        Err(e) => {
            // Don't leave a truncated file behind for the device to choke on.
            if let Some(partial) = e.partial {
                let _ = storage.delete(partial).await;
            }
            Err(MtpError::UploadFile(filename, e))
        }
    }
}

impl MtpClient {
    /// Lists MTP devices currently connected over USB, filtered to Garmin ones.
    pub async fn get_connected_devices(&self) -> Result<Vec<DeviceListItem>> {
        let mut res = Vec::new();

        let devices = MtpDevice::list_devices().map_err(MtpError::ListDevices)?;

        for device_info in devices {
            // Skip non-Garmin devices without opening (claiming) them.
            if device_info.vendor_id != constants::MTP_GARMIN_VENDOR_ID {
                continue;
            }

            let device = MtpDevice::open_by_location(device_info.location_id)
                .await
                .map_err(|e| MtpError::OpenDevice(device_info.location_id, e))?;

            let info = device.device_info();
            if info.manufacturer.to_uppercase() == constants::MTP_GARMIN_MANUFACTURER {
                res.push(DeviceListItem::from(info))
            }

            let _ = device.close().await;
        }

        Ok(res)
    }

    /// True if a device with serial number `serial` is currently connected over USB. Does not
    /// open (claim) the device.
    pub async fn is_device_connected(&self, serial: &str) -> Result<bool> {
        let devices = MtpDevice::list_devices().map_err(MtpError::ListDevices)?;

        Ok(devices
            .iter()
            .any(|d| d.serial_number.as_deref() == Some(serial)))
    }

    /// Downloads `.FIT` activity files newer than `date` from the device's `GARMIN/Activity` folder into a temp directory, returning their local paths.
    pub async fn download_activities_since(
        &self,
        serial: &str,
        date: String,
        dst_dir: PathBuf,
    ) -> Result<()> {
        let device = open_device(serial).await?;

        // Every fallible step below runs inside this block so that, regardless of how it
        // exits, `device.close()` below always runs exactly once.
        let result: Result<()> = async {
            let storages = device.storages().await.map_err(MtpError::Storage)?;
            let storage = storages
                .first()
                .ok_or_else(|| MtpError::NoStorageDevice(serial.to_string()))?;

            info!("Listing files...");
            let mut objs =
                list_garmin_folder(storage, serial, constants::MTP_GARMIN_ACTIVITY_FOLDER).await?;

            objs.retain(is_fit_file);
            info!("Found {} files", objs.len());
            objs.retain(|f| f.filename.split('.').next().unwrap() > date.as_str());

            if objs.is_empty() {
                info!("No pending files to import");
                return Ok(());
            }

            info!("Pending {} files", objs.len());

            download_files(storage, objs, &dst_dir).await?;
            Ok(())
        }
        .await;

        let _ = device.close().await;
        result
    }

    /// Downloads `.FIT` workout files.
    pub async fn download_workouts(&self, serial: &str, dst_dir: PathBuf) -> Result<()> {
        let device = open_device(serial).await?;

        // Every fallible step below runs inside this block so that, regardless of how it
        // exits, `device.close()` below always runs exactly once.
        let result: Result<()> = async {
            let storages = device.storages().await.map_err(MtpError::Storage)?;
            let storage = storages
                .first()
                .ok_or_else(|| MtpError::NoStorageDevice(serial.to_string()))?;

            info!("Listing files...");
            let mut objs =
                list_garmin_folder(storage, serial, constants::MTP_GARMIN_WORKOUTS_FOLDER).await?;

            objs.retain(is_fit_file);
            info!("Found {} files", objs.len());

            download_files(storage, objs, &dst_dir).await?;
            Ok(())
        }
        .await;

        let _ = device.close().await;
        result
    }

    /// Deletes the workout file `file_name`, looking first in `Workouts` and then in `NewFiles`.
    pub async fn delete_workout(&self, serial: &str, file_name: &str) -> Result<()> {
        let device = open_device(serial).await?;

        let result: Result<()> = async {
            info!("Looking for .fit file '{}'", file_name);
            let matches =
                |o: &ObjectInfo| o.is_file() && o.filename.eq_ignore_ascii_case(file_name);

            let storages = device.storages().await.map_err(MtpError::Storage)?;
            let storage = storages
                .first()
                .ok_or_else(|| MtpError::NoStorageDevice(serial.to_string()))?;

            let objs =
                list_garmin_folder(storage, serial, constants::MTP_GARMIN_WORKOUTS_FOLDER).await?;

            let mut obj = objs.into_iter().find(matches);

            if obj.is_none() {
                let objs =
                    list_garmin_folder(storage, serial, constants::MTP_GARMIN_NEW_FILES_FOLDER)
                        .await?;

                obj = objs.into_iter().find(matches);
            }

            if let Some(obj) = obj {
                delete_file(storage, obj).await?;
            } else {
                info!("Not found .fit file");
            }

            Ok(())
        }
        .await;

        let _ = device.close().await;
        result
    }

    /// Uploads the local file `src` into the device's `GARMIN/<folder>` folder, replacing any file
    /// with the same name.
    pub async fn upload_file(&self, serial: &str, folder: &str, src: &Path) -> Result<()> {
        let device = open_device(serial).await?;

        // Every fallible step below runs inside this block so that, regardless of how it
        // exits, `device.close()` below always runs exactly once.
        let result: Result<()> = async {
            let storages = device.storages().await.map_err(MtpError::Storage)?;
            let storage = storages
                .first()
                .ok_or_else(|| MtpError::NoStorageDevice(serial.to_string()))?;

            let target = find_garmin_folder(storage, serial, folder).await?;
            upload_file(storage, target.handle, src).await?;
            Ok(())
        }
        .await;

        let _ = device.close().await;
        result
    }

    /// Uploads a workout `.FIT` file into `GARMIN/NewFiles`, from where the device validates and
    /// imports it into `GARMIN/Workouts` once it is disconnected.
    pub async fn upload_workout<P>(&self, serial: &str, src: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        self.upload_file(serial, constants::MTP_GARMIN_NEW_FILES_FOLDER, src.as_ref())
            .await
    }
}
