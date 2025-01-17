// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use super::{
  configure_cargo, delete_codegen_vars, device_prompt, ensure_init, env, open_and_wait,
  setup_dev_config, with_config, MobileTarget,
};
use crate::{
  dev::Options as DevOptions,
  helpers::{config::get as get_config, flock},
  interface::{AppSettings, Interface, MobileOptions, Options as InterfaceOptions},
  mobile::{write_options, CliOptions, DevChild, DevProcess},
  Result,
};
use clap::{ArgAction, Parser};

use tauri_mobile::{
  android::{
    config::{Config as AndroidConfig, Metadata as AndroidMetadata},
    device::Device,
    env::Env,
    target::Target,
  },
  config::app::App,
  opts::{FilterLevel, NoiseLevel, Profile},
  target::TargetTrait,
};

use std::env::set_var;

const WEBVIEW_CLIENT_CLASS_EXTENSION: &str = "
    @android.annotation.SuppressLint(\"WebViewClientOnReceivedSslError\")
    override fun onReceivedSslError(view: WebView?, handler: SslErrorHandler, error: android.net.http.SslError) {
        handler.proceed()
    }
";
const WEBVIEW_CLASS_INIT: &str =
  "this.settings.mixedContentMode = android.webkit.WebSettings.MIXED_CONTENT_ALWAYS_ALLOW";

#[derive(Debug, Clone, Parser)]
#[clap(about = "Android dev")]
pub struct Options {
  /// List of cargo features to activate
  #[clap(short, long, action = ArgAction::Append, num_args(0..))]
  pub features: Option<Vec<String>>,
  /// Exit on panic
  #[clap(short, long)]
  exit_on_panic: bool,
  /// JSON string or path to JSON file to merge with tauri.conf.json
  #[clap(short, long)]
  pub config: Option<String>,
  /// Disable the file watcher
  #[clap(long)]
  pub no_watch: bool,
  /// Disable the dev server for static files.
  #[clap(long)]
  pub no_dev_server: bool,
  /// Open Android Studio instead of trying to run on a connected device
  #[clap(short, long)]
  pub open: bool,
  /// Runs on the given device name
  pub device: Option<String>,
}

impl From<Options> for DevOptions {
  fn from(options: Options) -> Self {
    Self {
      runner: None,
      target: None,
      features: options.features,
      exit_on_panic: options.exit_on_panic,
      config: options.config,
      release_mode: false,
      args: Vec::new(),
      no_watch: options.no_watch,
      no_dev_server: options.no_dev_server,
    }
  }
}

pub fn command(options: Options, noise_level: NoiseLevel) -> Result<()> {
  delete_codegen_vars();
  with_config(
    Some(Default::default()),
    |app, config, metadata, _cli_options| {
      set_var(
        "WRY_RUSTWEBVIEWCLIENT_CLASS_EXTENSION",
        WEBVIEW_CLIENT_CLASS_EXTENSION,
      );
      set_var("WRY_RUSTWEBVIEW_CLASS_INIT", WEBVIEW_CLASS_INIT);
      ensure_init(config.project_dir(), MobileTarget::Android)?;
      run_dev(options, app, config, metadata, noise_level).map_err(Into::into)
    },
  )
  .map_err(Into::into)
}

fn run_dev(
  mut options: Options,
  app: &App,
  config: &AndroidConfig,
  metadata: &AndroidMetadata,
  noise_level: NoiseLevel,
) -> Result<()> {
  setup_dev_config(&mut options.config)?;
  let mut env = env()?;
  let device = if options.open {
    None
  } else {
    match device_prompt(&env, options.device.as_deref()) {
      Ok(d) => Some(d),
      Err(e) => {
        log::error!("{e}");
        None
      }
    }
  };

  let mut dev_options: DevOptions = options.clone().into();
  let target_triple = device
    .as_ref()
    .map(|d| d.target().triple.to_string())
    .unwrap_or_else(|| Target::all().values().next().unwrap().triple.into());
  dev_options.target = Some(target_triple.clone());
  let mut interface = crate::dev::setup(&mut dev_options, true)?;

  let interface_options = InterfaceOptions {
    debug: !dev_options.release_mode,
    target: dev_options.target.clone(),
    ..Default::default()
  };

  let app_settings = interface.app_settings();
  let bin_path = app_settings.app_binary_path(&interface_options)?;
  let out_dir = bin_path.parent().unwrap();
  let _lock = flock::open_rw(out_dir.join("lock").with_extension("android"), "Android")?;

  configure_cargo(app, Some((&mut env, config)))?;

  // run an initial build to initialize plugins
  let target = Target::all()
    .values()
    .find(|t| t.triple == target_triple)
    .unwrap_or_else(|| Target::all().values().next().unwrap());
  target.build(config, metadata, &env, noise_level, true, Profile::Debug)?;

  let open = options.open;
  let exit_on_panic = options.exit_on_panic;
  let no_watch = options.no_watch;
  interface.mobile_dev(
    MobileOptions {
      debug: true,
      features: options.features,
      args: Vec::new(),
      config: options.config,
      no_watch: options.no_watch,
    },
    |options| {
      let cli_options = CliOptions {
        features: options.features.clone(),
        args: options.args.clone(),
        noise_level,
        vars: Default::default(),
      };
      let _handle = write_options(
        &get_config(options.config.as_deref())?
          .lock()
          .unwrap()
          .as_ref()
          .unwrap()
          .tauri
          .bundle
          .identifier,
        cli_options,
      )?;

      if open {
        open_and_wait(config, &env)
      } else if let Some(device) = &device {
        match run(device, options, config, &env, metadata, noise_level) {
          Ok(c) => {
            crate::dev::wait_dev_process(c.clone(), move |status, reason| {
              crate::dev::on_app_exit(status, reason, exit_on_panic, no_watch)
            });
            Ok(Box::new(c) as Box<dyn DevProcess>)
          }
          Err(e) => {
            crate::dev::kill_before_dev_process();
            Err(e.into())
          }
        }
      } else {
        open_and_wait(config, &env)
      }
    },
  )
}

#[derive(Debug, thiserror::Error)]
enum RunError {
  #[error("{0}")]
  RunFailed(String),
}

fn run(
  device: &Device<'_>,
  options: MobileOptions,
  config: &AndroidConfig,
  env: &Env,
  metadata: &AndroidMetadata,
  noise_level: NoiseLevel,
) -> Result<DevChild, RunError> {
  let profile = if options.debug {
    Profile::Debug
  } else {
    Profile::Release
  };

  let build_app_bundle = metadata.asset_packs().is_some();

  device
    .run(
      config,
      env,
      noise_level,
      profile,
      Some(match noise_level {
        NoiseLevel::Polite => FilterLevel::Info,
        NoiseLevel::LoudAndProud => FilterLevel::Debug,
        NoiseLevel::FranklyQuitePedantic => FilterLevel::Verbose,
      }),
      build_app_bundle,
      false,
      ".MainActivity".into(),
    )
    .map(DevChild::new)
    .map_err(|e| RunError::RunFailed(e.to_string()))
}
