//! Debug-only command line options, not compiled in release builds.

use std::path::Path;

use crate::fit::parser::FitParser;

fn decode_files<P>(paths: &[P])
where
    P: AsRef<Path>,
{
    for path in paths {
        let res: Result<(), Box<dyn std::error::Error>> = match FitParser::from_file(path) {
            Ok(stream) => match stream.debug_dump() {
                Ok(_) => Ok(()),
                Err(e) => Err(e),
            },
            Err(e) => Err(Box::new(e)),
        };
        if let Err(e) = res {
            eprintln!(
                "Error handling '{}': \n  {}",
                path.as_ref().display(),
                e.as_ref()
            )
        }
    }
}

fn encode_files<P>(paths: &[P])
where
    P: AsRef<Path>,
{
    for path in paths {
        if let Err(e) = crate::fit::parser::debug_encode(path) {
            eprintln!("Error handling '{}': \n  {}", path.as_ref().display(), e)
        }
    }
}

/// Incremental dump of the connected Garmin device into `./dump`.
fn dump_device() {
    if let Err(e) = crate::mtp::dump::run(Path::new("dump")) {
        eprintln!("Error dumping device: {e}");
    }
}

/// Removes a dumped file from the connected Garmin device and from `./dump`. Returns the exit code.
fn delete_dumped_file(path: &str) -> i32 {
    match crate::mtp::dump::delete(Path::new(path), Path::new("dump")) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Error deleting '{path}': {e}");
            1
        }
    }
}

/// Lists a path of the connected Garmin device. Returns the exit code.
fn list_device_path(path: &str) -> i32 {
    match crate::mtp::dump::ls(path) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Error listing '{path}': {e}");
            1
        }
    }
}

/// Runs the debug option in `args`, returning the exit code, or `None` if there is no such option
/// and the app has to start.
pub fn run_option(args: &[String]) -> Option<i32> {
    let option = args.get(1)?;
    let params = &args[2..];

    match option.as_str() {
        "--decode" if !params.is_empty() => {
            decode_files(params);
            Some(0)
        }
        "--encode" if !params.is_empty() => {
            encode_files(params);
            Some(0)
        }
        "--dump" => {
            dump_device();
            Some(0)
        }
        "--dump-delete" => Some(match params.first() {
            Some(path) => delete_dumped_file(path),
            None => {
                eprintln!("Bad usage: --dump-delete <path of a file inside ./dump>");
                1
            }
        }),
        "--ls-mtp" => Some(list_device_path(params.first().map_or("", String::as_str))),
        _ => None,
    }
}
