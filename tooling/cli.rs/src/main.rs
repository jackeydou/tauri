use std::env::args_os;
use std::ffi::OsStr;
use std::path::Path;
use std::process::exit;

fn main() -> tauri_cli::Result<()> {
  let mut args = args_os();
  let bin_name = match args
    .next()
    .as_deref()
    .map(Path::new)
    .and_then(Path::file_stem)
    .and_then(OsStr::to_str)
  {
    Some("cargo-tauri") => {
      if args.by_ref().peekable().peek().and_then(|s| s.to_str()) == Some("tauri") {
        // remove the extra cargo external tools subcommand
        args.next();

        Some("cargo tauri".into())
      } else {
        Some("cargo-tauri".into())
      }
    }
    Some(stem) => Some(stem.to_string()),
    None => {
      eprintln!("cargo-tauri wrapper unable to read first argument");
      exit(1);
    }
  };

  tauri_cli::run(args, bin_name)
}
