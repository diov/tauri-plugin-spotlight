use crate::Error;
use crate::{PluginConfig, WindowConfig};

use core::fmt;
use objc_id::ShareId;
use std::{
    collections::HashMap,
    sync::{Mutex, RwLock},
};
use tauri::{GlobalShortcutManager, Manager, Window, Wry};
use tauri_nspanel::cocoa::appkit::{NSMainMenuWindowLevel, NSWindowCollectionBehavior};
use tauri_nspanel::panel_delegate;
use tauri_nspanel::raw_nspanel::RawNSPanel;

#[allow(non_upper_case_globals)]
const NSWindowStyleMaskNonActivatingPanel: i32 = 1 << 7;

struct RawNSPanelWrapper(ShareId<RawNSPanel>);

impl fmt::Debug for RawNSPanelWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RawNSPanelWraper")
    }
}

#[derive(Default, Debug)]
pub struct SpotlightManager {
    pub config: PluginConfig,
    panels: RwLock<HashMap<String, Mutex<RawNSPanelWrapper>>>,
}

impl SpotlightManager {
    pub fn new(config: PluginConfig) -> Self {
        let mut manager = Self::default();
        manager.config = config;
        manager
    }

    fn get_window_config(&self, window: &Window<Wry>) -> Option<WindowConfig> {
        if let Some(window_configs) = self.config.windows.clone() {
            for window_config in window_configs {
                if window.label() == window_config.label {
                    return Some(window_config.clone());
                }
            }
        }
        None
    }

    pub fn init_spotlight_window(&self, window: &Window<Wry>) -> Result<(), Error> {
        let window_config = match self.get_window_config(&window) {
            Some(window_config) => window_config,
            None => return Ok(()),
        };
        let label = window.label();
        let mut map = self
            .panels
            .write()
            .map_err(|_| Error::RwLock(String::from("failed to write registered panels")))?;
        if map.get(label).is_none() {
            let panel = window_to_panel(window)?;
            setup_panel_for_window(window, &panel, &window_config)?;
            let wrapper = RawNSPanelWrapper(panel);
            map.insert(label.into(), Mutex::new(wrapper));

            register_shortcut_for_window(&window, &window_config)?;
            register_close_shortcut(&window)?;
        }
        Ok(())
    }

    pub fn get_panel(&self, label: &str) -> Result<ShareId<RawNSPanel>, Error> {
        let map = self
            .panels
            .read()
            .map_err(|_| Error::RwLock(String::from("failed to read registered panels")))?;
        if let Some(panel) = map.get(label) {
            let panel = panel
                .lock()
                .map_err(|_| Error::Mutex(String::from("failed to lock panel")))?;
            Ok(panel.0.clone())
        } else {
            Err(Error::Other(String::from("panel not found")))
        }
    }

    pub fn show(&self, label: &str) -> Result<(), Error> {
        if let Ok(panel) = self.get_panel(label) {
            if !panel.is_visible() {
                panel.show();
            }
        }
        Ok(())
    }

    pub fn hide(&self, label: &str) -> Result<(), Error> {
        if let Ok(panel) = self.get_panel(label) {
            if panel.is_visible() {
                panel.order_out(None);
            }
        }
        Ok(())
    }
}

fn window_to_panel(window: &Window<Wry>) -> Result<ShareId<RawNSPanel>, Error> {
    let panel = RawNSPanel::from_window(window.to_owned());
    Ok(panel.share())
}

fn setup_panel_for_window(
    window: &Window<Wry>,
    panel: &ShareId<RawNSPanel>,
    window_config: &WindowConfig,
) -> Result<(), Error> {
    let app_handle = window.app_handle();

    let window_level = window_config
        .macos_window_level
        .unwrap_or(NSMainMenuWindowLevel + 1);
    panel.set_level(window_level);

    panel.set_style_mask(NSWindowStyleMaskNonActivatingPanel);
    panel.set_collection_behaviour(
        NSWindowCollectionBehavior::NSWindowCollectionBehaviorTransient
            | NSWindowCollectionBehavior::NSWindowCollectionBehaviorMoveToActiveSpace
            | NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary,
    );

    let auto_hide = window_config.auto_hide.unwrap_or(true);
    let panel_delegate = panel_delegate!(SpotlightPanelDelegate {
        window_did_resign_key
    });
    let label = window.label().to_owned();
    panel_delegate.set_listener(Box::new(move |delegate_name: String| {
        match delegate_name.as_str() {
            "window_did_resign_key" => {
                if auto_hide {
                    let manager = app_handle.state::<SpotlightManager>();
                    let panel = manager.get_panel(&label).unwrap();
                    panel.order_out(None);
                }
            }
            _ => (),
        }
    }));
    panel.set_delegate(panel_delegate);

    Ok(())
}

fn register_shortcut_for_window(
    window: &Window<Wry>,
    window_config: &WindowConfig,
) -> Result<(), Error> {
    let shortcut = match window_config.shortcut.clone() {
        Some(shortcut) => shortcut,
        None => return Ok(()),
    };
    let window = window.to_owned();
    let app_handle = window.app_handle();
    let mut shortcut_manager = app_handle.global_shortcut_manager();
    shortcut_manager
        .register(&shortcut, move || {
            let manager = app_handle.state::<SpotlightManager>();
            let panel = manager.get_panel(window.label()).unwrap();
            if panel.is_visible() {
                panel.order_out(None);
            } else {
                panel.show();
            }
        })
        .map_err(|_| Error::Other(String::from("failed to register shortcut")))?;
    Ok(())
}

fn register_close_shortcut(window: &Window<Wry>) -> Result<(), Error> {
    let window = window.to_owned();
    let mut shortcut_manager = window.app_handle().global_shortcut_manager();
    let app_handle = window.app_handle();
    let manager = app_handle.state::<SpotlightManager>();
    if let Some(close_shortcut) = &manager.config.global_close_shortcut {
        if let Ok(registered) = shortcut_manager.is_registered(&close_shortcut) {
            if !registered {
                shortcut_manager
                    .register(&close_shortcut, move || {
                        let app_handle = window.app_handle();
                        let state = app_handle.state::<SpotlightManager>();
                        let labels = if let Some(ref windows) = state.config.windows {
                            windows.iter().map(|window| window.label.clone()).collect()
                        } else {
                            vec![]
                        };
                        for label in labels {
                            if let Ok(panel) = state.get_panel(&label) {
                                panel.order_out(None);
                            }
                        }
                    })
                    .map_err(tauri::Error::Runtime)?;
            }
        } else {
            return Err(Error::Other(String::from("Shortcut already registered")));
        }
    }
    Ok(())
}
