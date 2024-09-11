#[cfg_attr(target_os = "macos", path = "spotlight_macos/mod.rs")]
#[cfg_attr(not(target_os = "macos"), path = "spotlight_others.rs")]
mod spotlight;
mod error;
mod config;

pub use config::{PluginConfig, WindowConfig};
pub use error::Error;

use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Wry, Runtime, State,
};

pub trait ManagerExt<R: Runtime> {
    fn spotlight(&self) -> State<'_, spotlight::SpotlightManager>;
}

impl<R: Runtime, T: Manager<R>> ManagerExt<R> for T {
  fn spotlight(&self) -> State<'_, spotlight::SpotlightManager> {
    self.state::<spotlight::SpotlightManager>()
  }
}

// #[tauri::command]
// #[cfg(target_os = "macos")]
// fn show(manager: State<'_, spotlight::SpotlightManager>, label: &str) -> Result<(), String> {
//     manager.show(label).map_err(|err| format!("{:?}", err))
// }

// #[tauri::command]
// #[cfg(target_os = "macos")]
// fn hide(manager: State<'_, spotlight::SpotlightManager>, label: &str) -> Result<(), String> {
//     manager.hide(label).map_err(|err| format!("{:?}", err))
// }

// #[tauri::command]
// #[cfg(target_os = "windows")]
// fn show(manager: State<'_, spotlight::SpotlightManager>, label: &str) -> Result<(), String> {
//     if let Some(window) = app.get_window(label) {
//         let manager = app.spotlight();
//         manager.show(window).map_err(|err| format!("{:?}", err))
//     } else {
//         return Err(format!("Window with label '{}' not found", label));
//     }
// }

// #[tauri::command]
// #[cfg(target_os = "windows")]
// fn hide(manager: State<'_, spotlight::SpotlightManager>, label: &str) -> Result<(), String> {
//     if let Some(window) = app.get_window(label) {
//         let manager = app.spotlight();
//         manager.hide(window).map_err(|err| format!("{:?}", err))
//     } else {
//         return Err(format!("Window with label '{}' not found", label));
//     }
// }

pub fn init(spotlight_config: Option<PluginConfig>) -> TauriPlugin<Wry, Option<PluginConfig>> {
    Builder::<Wry, Option<PluginConfig>>::new("spotlight")
        // .invoke_handler(tauri::generate_handler![show, hide])
        .setup(|app| {
            app.manage(spotlight::SpotlightManager::new(spotlight_config.unwrap_or(PluginConfig::default())));
            Ok(())
        })
        .on_webview_ready(move |window| {
            let app_handle = window.app_handle();
            app_handle.spotlight().init_spotlight_window(&window).unwrap();
        })
        .build()
}
