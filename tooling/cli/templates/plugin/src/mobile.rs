{{#if license_header}}
{{ license_header }}
{{/if}}
use serde::de::DeserializeOwned;
use tauri::{
  plugin::{PluginApi, PluginHandle},
  AppHandle, Runtime,
};

use crate::models::*;

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "{{ android_package_id }}";

#[cfg(target_os = "ios")]
extern "C" {
  fn init_plugin_{{ plugin_name }}(webview: tauri::cocoa::base::id);
}

// initializes the Kotlin or Swift plugin classes
pub fn init<R: Runtime, C: DeserializeOwned>(
  _app: &AppHandle<R>,
  api: PluginApi<R, C>,
) -> crate::Result<{{ plugin_name_pascal_case }}<R>> {
  #[cfg(target_os = "android")]
  let handle = api.register_android_plugin(PLUGIN_IDENTIFIER, "ExamplePlugin")?;
  #[cfg(target_os = "ios")]
  let handle = api.register_ios_plugin(init_plugin_{{ plugin_name }})?;
  Ok({{ plugin_name_pascal_case }}(handle))
}

/// Access to the {{ plugin_name }} APIs.
pub struct {{ plugin_name_pascal_case }}<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> {{ plugin_name_pascal_case }}<R> {
  pub fn ping(&self, payload: PingRequest) -> crate::Result<PingResponse> {
    self
      .0
      .run_mobile_plugin("ping", payload)
      .map_err(Into::into)
  }
}
