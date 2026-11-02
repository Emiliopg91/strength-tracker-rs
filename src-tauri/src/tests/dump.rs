use std::{
    collections::HashSet,
    fs::{self, File},
    path::{Path, PathBuf},
    time::{Duration, UNIX_EPOCH},
};

use crate::mtp::dump::{device_path_components, dump_relative_components, is_unchanged, prune_dir};

/// Creates a dump directory holding `GARMIN/Workouts/a.fit`, plus `outside.fit` next to it.
fn setup(test: &str) -> (PathBuf, PathBuf) {
    let base = std::env::temp_dir().join(format!("gtrs-test-dump-{test}-{}", std::process::id()));
    let root = base.join("dump");
    fs::create_dir_all(root.join("GARMIN/Workouts")).unwrap();
    fs::write(root.join("GARMIN/Workouts/a.fit"), b"a").unwrap();
    fs::write(base.join("outside.fit"), b"b").unwrap();
    (base, root)
}

#[test]
fn file_inside_the_dump_gives_its_components() {
    let (base, root) = setup("inside");

    let components = dump_relative_components(&root.join("GARMIN/Workouts/a.fit"), &root).unwrap();

    assert_eq!(components, ["GARMIN", "Workouts", "a.fit"]);
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn path_is_normalized_before_comparing() {
    let (base, root) = setup("normalized");

    let path = root.join("GARMIN/../GARMIN/Workouts/a.fit");
    let components = dump_relative_components(&path, &root).unwrap();

    assert_eq!(components, ["GARMIN", "Workouts", "a.fit"]);
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn file_outside_the_dump_is_rejected() {
    let (base, root) = setup("outside");

    assert!(dump_relative_components(&base.join("outside.fit"), &root).is_err());
    assert!(dump_relative_components(&root.join("../outside.fit"), &root).is_err());
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn folder_is_rejected() {
    let (base, root) = setup("folder");

    assert!(dump_relative_components(&root.join("GARMIN"), &root).is_err());
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn missing_file_is_rejected() {
    let (base, root) = setup("missing");

    assert!(dump_relative_components(Path::new("/nonexistent/a.fit"), &root).is_err());
    assert!(dump_relative_components(&root.join("GARMIN/b.fit"), &root).is_err());
    fs::remove_dir_all(base).unwrap();
}

fn keep(names: &[&str]) -> HashSet<String> {
    names.iter().map(|n| n.to_string()).collect()
}

#[test]
fn prune_deletes_files_and_folders_missing_on_the_device() {
    let (base, root) = setup("prune");
    fs::write(root.join("gone.fit"), b"c").unwrap();
    fs::create_dir(root.join("gone_folder")).unwrap();
    fs::write(root.join("gone_folder/d.fit"), b"d").unwrap();

    let deleted = prune_dir(&root, &keep(&["GARMIN"])).unwrap();

    assert_eq!(deleted, 2);
    assert!(root.join("GARMIN/Workouts/a.fit").exists());
    assert!(!root.join("gone.fit").exists());
    assert!(!root.join("gone_folder").exists());
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn prune_keeps_everything_listed() {
    let (base, root) = setup("prune-keep");

    assert_eq!(prune_dir(&root, &keep(&["GARMIN"])).unwrap(), 0);
    assert!(root.join("GARMIN/Workouts/a.fit").exists());
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn file_is_unchanged_only_with_same_size_and_date() {
    let (base, root) = setup("unchanged");
    let path = root.join("GARMIN/Workouts/a.fit");
    let date = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    let other_date = date + Duration::from_secs(60);
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(date)
        .unwrap();
    let local = fs::metadata(&path).unwrap();

    assert!(is_unchanged(&local, 1, Some(date)));
    assert!(!is_unchanged(&local, 2, Some(date)));
    assert!(!is_unchanged(&local, 1, Some(other_date)));
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn without_device_date_only_the_size_counts() {
    let (base, root) = setup("no-date");
    let local = fs::metadata(root.join("GARMIN/Workouts/a.fit")).unwrap();

    assert!(is_unchanged(&local, 1, None));
    assert!(!is_unchanged(&local, 2, None));
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn folder_is_never_an_unchanged_file() {
    let (base, root) = setup("folder-unchanged");
    let local = fs::metadata(root.join("GARMIN")).unwrap();

    assert!(!is_unchanged(&local, local.len(), None));
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn device_path_ignores_extra_separators() {
    let expected = ["GARMIN", "Workouts"];

    assert_eq!(device_path_components("GARMIN/Workouts"), expected);
    assert_eq!(device_path_components("/GARMIN//Workouts/"), expected);
    assert_eq!(device_path_components("./GARMIN/./Workouts"), expected);
}

#[test]
fn device_root_has_no_components() {
    assert!(device_path_components("").is_empty());
    assert!(device_path_components("/").is_empty());
    assert!(device_path_components(".").is_empty());
}
