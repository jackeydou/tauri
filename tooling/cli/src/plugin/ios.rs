// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use crate::{helpers::template, Result};
use clap::{Parser, Subcommand};
use handlebars::Handlebars;

use std::{
  collections::BTreeMap,
  env::current_dir,
  ffi::OsStr,
  fs::{create_dir_all, File},
  path::{Component, PathBuf},
};

#[derive(Parser)]
#[clap(
  author,
  version,
  about = "Manage the iOS project for Tauri plugins",
  subcommand_required(true),
  arg_required_else_help(true)
)]
pub struct Cli {
  #[clap(subcommand)]
  command: Commands,
}

#[derive(Subcommand)]
enum Commands {
  Add(AddOptions),
}

#[derive(Debug, Parser)]
#[clap(about = "Adds the iOS project to an existing Tauri plugin")]
pub struct AddOptions {
  /// Name of your Tauri plugin. Must match the current plugin's name.
  #[clap(short = 'n', long = "name")]
  plugin_name: String,
  /// The output directory.
  #[clap(short, long)]
  #[clap(default_value_t = current_dir().expect("failed to read cwd").to_string_lossy().into_owned())]
  out_dir: String,
}

pub fn command(cli: Cli) -> Result<()> {
  match cli.command {
    Commands::Add(options) => {
      let out_dir = PathBuf::from(options.out_dir);
      if out_dir.join("ios").exists() {
        return Err(anyhow::anyhow!("ios folder already exists"));
      }

      let handlebars = Handlebars::new();

      let mut data = BTreeMap::new();
      super::init::plugin_name_data(&mut data, &options.plugin_name);

      let mut created_dirs = Vec::new();
      template::render_with_generator(
        &handlebars,
        &data,
        &super::init::TEMPLATE_DIR,
        &out_dir,
        &mut |path| {
          let mut components = path.components();
          let root = components.next().unwrap();
          if let Component::Normal(component) = root {
            if component == OsStr::new("ios") {
              let path = out_dir.join(path);
              let parent = path.parent().unwrap().to_path_buf();
              if !created_dirs.contains(&parent) {
                create_dir_all(&parent)?;
                created_dirs.push(parent);
              }
              return File::create(path).map(Some);
            }
          }

          Ok(None)
        },
      )?;

      let metadata = super::init::crates_metadata()?;

      let cargo_toml_addition = format!(
        r#"
[build-dependencies]
tauri-build = "{}"
"#,
        metadata.tauri_build
      );
      let build_file = super::init::TEMPLATE_DIR
        .get_file("build.rs")
        .unwrap()
        .contents_utf8()
        .unwrap();
      let init_fn = format!(
        r#"
#[cfg(target_os = "ios")]
extern "C" {{
  fn init_plugin_{name}(webview: tauri::cocoa::base::id);
}}

pub fn init<R: Runtime>() -> TauriPlugin<R> {{
  Builder::new("{name}")
    .setup(|app| {{
      #[cfg(target_os = "ios")]
      app.initialize_ios_plugin(init_plugin_{name})?;
      Ok(())
    }})
    .build()
}}
"#,
        name = options.plugin_name,
      );

      log::info!("iOS project added");
      println!("You must add the following to the Cargo.toml file:\n{cargo_toml_addition}",);
      println!("You must add the following code to the build.rs file:\n\n{build_file}",);
      println!(
        "Your plugin's init function under src/lib.rs must initialize the iOS plugin:\n{init_fn}"
      );
    }
  }

  Ok(())
}
