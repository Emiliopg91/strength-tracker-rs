//! Debug-only incremental dump of the connected Garmin device, launched with `--dump`.
//!
//! The device tree is mirrored under the target directory: new and modified files are downloaded,
//! and whatever is no longer on the device is deleted locally. A file is unchanged if its size and
//! modification time match, which downloads copy from the device. Downloads go to a `.part` file
//! first, so an interrupted run can be simply repeated. Map images (`.img`, several GB in total)
//! are never downloaded.
//!
//! `--dump-delete` removes a dumped file from the device and then from the dump.
//!
//! `--ls-mtp` lists a path of the device, without touching anything.

use std::{
    collections::HashSet,
    error::Error,
    fs::{self, File, Metadata},
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use mtp_rs::{ByteRange, MtpDevice, ObjectHandle, ObjectInfo, Storage, UtcOffset};

use crate::utils::constants;

type BoxResult<T> = Result<T, Box<dyn Error>>;

/// Extension of the Garmin map files, excluded from the dump because of their size.
const MAP_EXTENSION: &str = "img";

#[derive(Default)]
struct Stats {
    downloaded: u64,
    skipped: u64,
    excluded: u64,
    deleted: u64,
    failed: u64,
    bytes: u64,
}

/// Turns a device-provided name into a single safe path component.
fn safe_name(name: &str) -> String {
    match name {
        "" | "." | ".." => "_".to_string(),
        _ => name.replace(['/', '\\'], "_"),
    }
}

/// Modification time reported by the device. Dates are zoneless, so UTC only keeps them comparable.
fn device_modified(obj: &ObjectInfo) -> Option<SystemTime> {
    let secs = obj
        .modified?
        .to_unix_seconds_with_fallback(UtcOffset::UTC)?;
    Some(UNIX_EPOCH + Duration::from_secs(u64::try_from(secs).ok()?))
}

/// Whether the local file matches the device one. Without device date only the size is comparable.
pub fn is_unchanged(local: &Metadata, size: u64, modified: Option<SystemTime>) -> bool {
    local.is_file()
        && local.len() == size
        && modified.is_none_or(|m| local.modified().is_ok_and(|l| l == m))
}

/// Deletes the file or folder `path`, whichever it is.
fn remove_path(path: &Path) -> std::io::Result<()> {
    if fs::symlink_metadata(path)?.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

/// Deletes the entries of `dir` not named in `keep`, returning how many were deleted.
pub fn prune_dir(dir: &Path, keep: &HashSet<String>) -> std::io::Result<u64> {
    let mut deleted = 0;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if !keep.contains(entry.file_name().to_string_lossy().as_ref()) {
            println!("[del ] {}", entry.path().display());
            remove_path(&entry.path())?;
            deleted += 1;
        }
    }
    Ok(deleted)
}

async fn download_file(
    storage: &Storage,
    handle: ObjectHandle,
    size: u64,
    modified: Option<SystemTime>,
    dest: &Path,
) -> BoxResult<u64> {
    // The device answers a download of an empty file with an error, so there is nothing to fetch.
    if size == 0 {
        let file = File::create(dest)?;
        if let Some(modified) = modified {
            file.set_modified(modified)?;
        }
        return Ok(0);
    }

    let part = PathBuf::from(format!("{}.part", dest.display()));
    let result = async {
        let mut file = File::create(&part)?;
        let mut download = storage.download(handle, ByteRange::Full).await?;
        while let Some(chunk) = download.next_chunk().await {
            file.write_all(&chunk?)?;
        }
        file.flush()?;
        if let Some(modified) = modified {
            file.set_modified(modified)?;
        }
        fs::rename(&part, dest)?;
        BoxResult::Ok(download.bytes_received())
    }
    .await;

    if result.is_err() {
        let _ = fs::remove_file(&part);
    }
    result
}

async fn dump_storage(storage: &Storage, root: &Path, stats: &mut Stats) -> BoxResult<()> {
    fs::create_dir_all(root)?;
    let mut pending: Vec<(Option<ObjectHandle>, PathBuf)> = vec![(None, root.to_path_buf())];

    while let Some((parent, dir)) = pending.pop() {
        let objs = match storage.list_objects(parent).await {
            Ok(objs) => objs,
            Err(e) => {
                eprintln!("[fail] listing {}: {e}", dir.display());
                stats.failed += 1;
                continue;
            }
        };

        // An empty root is far more likely a device glitch than a wiped device
        if parent.is_none() && objs.is_empty() {
            return Err(format!("{} listed as empty, nothing synced", dir.display()).into());
        }

        // Only reached with a successful listing, so a failed one never deletes anything
        let keep = objs.iter().map(|o| safe_name(&o.filename)).collect();
        stats.deleted += prune_dir(&dir, &keep)?;

        for obj in objs {
            let path = dir.join(safe_name(&obj.filename));
            let local = fs::symlink_metadata(&path).ok();
            // Replaced by the other kind on the device: a folder for a file or the other way round
            if local
                .as_ref()
                .is_some_and(|m| m.is_dir() != obj.is_folder())
            {
                println!("[del ] {}", path.display());
                remove_path(&path)?;
                stats.deleted += 1;
            }

            if obj.is_folder() {
                fs::create_dir_all(&path)?;
                pending.push((Some(obj.handle), path));
                continue;
            }

            if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case(MAP_EXTENSION))
            {
                stats.excluded += 1;
                continue;
            }

            let modified = device_modified(&obj);
            if fs::metadata(&path).is_ok_and(|m| is_unchanged(&m, obj.size, modified)) {
                stats.skipped += 1;
                continue;
            }

            println!("[get ] {} ({} bytes)", path.display(), obj.size);
            match download_file(storage, obj.handle, obj.size, modified, &path).await {
                Ok(size) => {
                    stats.downloaded += 1;
                    stats.bytes += size;
                }
                Err(e) => {
                    eprintln!("[fail] {}: {e}", path.display());
                    stats.failed += 1;
                }
            }
        }
    }
    Ok(())
}

async fn open_garmin() -> BoxResult<MtpDevice> {
    for info in MtpDevice::list_devices()? {
        let device = MtpDevice::open_by_location(info.location_id).await?;
        if device.device_info().manufacturer.to_uppercase() == constants::MTP_GARMIN_MANUFACTURER {
            return Ok(device);
        }
        let _ = device.close().await;
    }
    Err("No Garmin MTP device connected".into())
}

async fn dump(out_dir: &Path) -> BoxResult<()> {
    let device = open_garmin().await?;
    println!(
        "Dumping {} {} into {}",
        device.device_info().manufacturer,
        device.device_info().model,
        out_dir.display()
    );

    let mut stats = Stats::default();
    let result = async {
        let storages = device.storages().await?;
        let multiple = storages.len() > 1;
        for storage in &storages {
            let root = if multiple {
                out_dir.join(safe_name(&storage.info().description))
            } else {
                out_dir.to_path_buf()
            };
            dump_storage(storage, &root, &mut stats).await?;
        }
        BoxResult::Ok(())
    }
    .await;
    let _ = device.close().await;

    println!(
        "Done: {} downloaded ({} bytes), {} unchanged, {} deleted, {} maps excluded, {} failed",
        stats.downloaded, stats.bytes, stats.skipped, stats.deleted, stats.excluded, stats.failed
    );
    result
}

/// Mirrors the connected Garmin device into `out_dir`, syncing only what changed.
pub fn run(out_dir: &Path) -> BoxResult<()> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(dump(out_dir))
}

/// Path components of the existing file `path` relative to the dump directory `root`.
pub fn dump_relative_components(path: &Path, root: &Path) -> BoxResult<Vec<String>> {
    let root = root.canonicalize()?;
    let path = path.canonicalize()?;
    if !path.is_file() {
        return Err(format!("{} is not a file", path.display()).into());
    }

    let relative = path
        .strip_prefix(&root)
        .map_err(|_| format!("{} is not inside {}", path.display(), root.display()))?;
    relative
        .iter()
        .map(|c| {
            c.to_str()
                .map(str::to_string)
                .ok_or_else(|| format!("Invalid file name in {}", relative.display()).into())
        })
        .collect()
}

/// Finds the folder at `components`, compared as `dump_storage` names them. `None` is the storage root.
async fn find_folder(storage: &Storage, components: &[String]) -> BoxResult<Option<ObjectHandle>> {
    let mut parent = None;
    for folder in components {
        let handle = storage
            .list_objects(parent)
            .await?
            .into_iter()
            .find(|o| o.is_folder() && safe_name(&o.filename) == *folder)
            .ok_or_else(|| format!("Folder {folder} not found on the device"))?
            .handle;
        parent = Some(handle);
    }
    Ok(parent)
}

/// Finds the file at `components`, compared as `dump_storage` names them, starting at the storage root.
async fn find_file(storage: &Storage, components: &[String]) -> BoxResult<ObjectHandle> {
    let (file, folders) = components.split_last().ok_or("Empty path")?;

    let parent = find_folder(storage, folders).await?;
    let handle = storage
        .list_objects(parent)
        .await?
        .into_iter()
        .find(|o| o.is_file() && safe_name(&o.filename) == *file)
        .ok_or_else(|| format!("File {file} not found on the device"))?
        .handle;
    Ok(handle)
}

/// Same layout as the dump: with several storages, the first component names the storage.
fn select_storage<'a>(
    storages: &'a [Storage],
    components: &'a [String],
) -> BoxResult<(&'a Storage, &'a [String])> {
    if storages.len() > 1 {
        let (name, rest) = components.split_first().ok_or("Empty path")?;
        let storage = storages
            .iter()
            .find(|s| safe_name(&s.info().description) == *name)
            .ok_or_else(|| format!("Storage {name} not found on the device"))?;
        Ok((storage, rest))
    } else {
        Ok((
            storages.first().ok_or("Device without storages")?,
            components,
        ))
    }
}

async fn delete_from_device(components: &[String]) -> BoxResult<()> {
    let device = open_garmin().await?;

    let result = async {
        let storages = device.storages().await?;
        let (storage, components) = select_storage(&storages, components)?;

        let handle = find_file(storage, components).await?;
        storage.delete(handle).await?;
        BoxResult::Ok(())
    }
    .await;
    let _ = device.close().await;

    result
}

/// Deletes the dumped file `path` from the connected Garmin device and, once deleted there, from
/// the dump directory `root`.
pub fn delete(path: &Path, root: &Path) -> BoxResult<()> {
    let components = dump_relative_components(path, root)?;

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(delete_from_device(&components))?;
    println!("Deleted from the device: {}", components.join("/"));

    fs::remove_file(path)?;
    println!("Deleted from the dump: {}", path.display());
    Ok(())
}

/// Components of a device path such as `/GARMIN/Workouts/`. The storage root has none.
pub fn device_path_components(path: &str) -> Vec<String> {
    path.split('/')
        .filter(|c| !c.is_empty() && *c != ".")
        .map(str::to_string)
        .collect()
}

/// The entries of the folder at `components`, or the entry itself if it is a file, as `ls` does.
async fn list_path(storage: &Storage, components: &[String]) -> BoxResult<Vec<ObjectInfo>> {
    let Some((last, folders)) = components.split_last() else {
        return Ok(storage.list_objects(None).await?);
    };

    let parent = find_folder(storage, folders).await?;
    let obj = storage
        .list_objects(parent)
        .await?
        .into_iter()
        .find(|o| safe_name(&o.filename) == *last)
        .ok_or_else(|| format!("{last} not found on the device"))?;
    if obj.is_folder() {
        Ok(storage.list_objects(Some(obj.handle)).await?)
    } else {
        Ok(vec![obj])
    }
}

fn format_entry(obj: &ObjectInfo) -> String {
    let kind = if obj.is_folder() { 'd' } else { '-' };
    let date = obj.modified.as_ref().map_or_else(
        || "-".repeat(19),
        |d| {
            format!(
                "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                d.year, d.month, d.day, d.hour, d.minute, d.second
            )
        },
    );
    format!("{kind} {:>12} {date} {}", obj.size, obj.filename)
}

async fn print_path(components: &[String]) -> BoxResult<()> {
    let device = open_garmin().await?;

    let result = async {
        let storages = device.storages().await?;
        // With several storages, the root of the dump layout holds one folder per storage
        if storages.len() > 1 && components.is_empty() {
            for storage in &storages {
                println!("{}", safe_name(&storage.info().description));
            }
            return BoxResult::Ok(());
        }

        let (storage, components) = select_storage(&storages, components)?;
        let mut objs = list_path(storage, components).await?;
        objs.sort_by(|a, b| a.filename.cmp(&b.filename));
        for obj in &objs {
            println!("{}", format_entry(obj));
        }
        BoxResult::Ok(())
    }
    .await;
    let _ = device.close().await;

    result
}

/// Prints the contents of `path` on the connected Garmin device, starting at the storage root.
pub fn ls(path: &str) -> BoxResult<()> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(print_path(&device_path_components(path)))
}
