// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

pub use anyhow::Result;

mod build;
mod dev;
mod helpers;
mod info;
mod init;
mod interface;
mod plugin;
mod signer;

use clap::{AppSettings, FromArgMatches, IntoApp, Parser, Subcommand};

use std::ffi::OsString;

#[derive(serde::Deserialize)]
pub struct VersionMetadata {
  tauri: String,
  #[serde(rename = "tauri-build")]
  tauri_build: String,
}

#[derive(Parser)]
#[clap(author, version, about, bin_name("cargo-tauri"))]
#[clap(global_setting(AppSettings::PropagateVersion))]
#[clap(global_setting(AppSettings::UseLongFormatForHelpSubcommand))]
#[clap(global_setting(AppSettings::NoBinaryName))]
#[clap(setting(AppSettings::SubcommandRequiredElseHelp))]
struct Cli {
  #[clap(subcommand)]
  command: Commands,
}

#[derive(Subcommand)]
enum Commands {
  Build(build::Options),
  Dev(dev::Options),
  Info(info::Options),
  Init(init::Options),
  Plugin(plugin::Cli),
  Signer(signer::Cli),
}

fn format_error<I: IntoApp>(err: clap::Error) -> clap::Error {
  let mut app = I::into_app();
  err.format(&mut app)
}

/// Run the Tauri CLI with the passed arguments.
///
/// The passed arguments should have the binary argument(s) stripped out before being passed.
///
/// e.g.
/// 1. `tauri-cli 1 2 3` -> `1 2 3`
/// 2. `cargo tauri 1 2 3` -> `1 2 3`
/// 3. `node tauri.js 1 2 3` -> `1 2 3`
///
/// The passed `bin_name` parameter should be how you want the help messages to display the command.
/// This defaults to `cargo-tauri`, but should be set to how the program was called, such as
/// `cargo tauri`.
pub fn run<I, A>(args: I, bin_name: Option<String>) -> Result<()>
where
  I: IntoIterator<Item = A>,
  A: Into<OsString> + Clone,
{
  let matches = match bin_name {
    Some(bin_name) => Cli::into_app().bin_name(bin_name),
    None => Cli::into_app(),
  }
  .get_matches_from(args);

  let res = Cli::from_arg_matches(&matches).map_err(format_error::<Cli>);
  let cli = match res {
    Ok(s) => s,
    Err(e) => e.exit(),
  };

  match cli.command {
    Commands::Build(options) => build::command(options)?,
    Commands::Dev(options) => dev::command(options)?,
    Commands::Info(options) => info::command(options)?,
    Commands::Init(options) => init::command(options)?,
    Commands::Plugin(cli) => plugin::command(cli)?,
    Commands::Signer(cli) => signer::command(cli)?,
  }

  Ok(())
}