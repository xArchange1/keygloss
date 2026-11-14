//! `ForegroundApp` via `NSWorkspace`. Apps are identified by bundle id.

use std::ptr::NonNull;
use std::sync::Mutex;

use block2::RcBlock;
use keygloss_core::platform::{AppId, AppInfo, ForegroundApp};
use objc2_app_kit::{
    NSApplicationActivationPolicy, NSRunningApplication, NSWorkspace, NSWorkspaceDidActivateApplicationNotification,
};
use objc2_foundation::NSNotification;

use crate::cf::on_main;

fn app_id(app: &NSRunningApplication) -> Option<AppId> {
    let id = app.bundleIdentifier().or_else(|| app.localizedName())?;
    Some(AppId::from_exe(&id.to_string()))
}

/// `ForegroundApp` implementation.
#[derive(Clone, Copy, Debug, Default)]
pub struct MacForegroundApp;

impl ForegroundApp for MacForegroundApp {
    fn foreground_app(&self) -> Option<AppId> {
        app_id(&*NSWorkspace::sharedWorkspace().frontmostApplication()?)
    }

    /// Observes `NSWorkspaceDidActivateApplicationNotification` (delivered on the main thread)
    /// for the life of the process; consecutive activations of one app are not reported.
    fn on_foreground_change(&self, callback: Box<dyn Fn(AppId) + Send + Sync>) {
        let last = Mutex::new(self.foreground_app());
        on_main(move || {
            let block = RcBlock::new(move |_: NonNull<NSNotification>| {
                let Some(app) = MacForegroundApp.foreground_app() else { return };
                let mut last = last.lock().unwrap_or_else(|p| p.into_inner());
                if last.as_ref() != Some(&app) {
                    *last = Some(app.clone());
                    callback(app);
                }
            });
            let center = NSWorkspace::sharedWorkspace().notificationCenter();
            let token = unsafe {
                center.addObserverForName_object_queue_usingBlock(
                    Some(NSWorkspaceDidActivateApplicationNotification),
                    None,
                    None,
                    &block,
                )
            };
            // Observes for the life of the process.
            std::mem::forget(token);
        });
    }

    fn running_apps(&self) -> Vec<AppInfo> {
        let mut apps: Vec<AppInfo> = Vec::new();
        for app in NSWorkspace::sharedWorkspace().runningApplications() {
            if app.activationPolicy() != NSApplicationActivationPolicy::Regular {
                continue;
            }
            let Some(id) = app_id(&app) else { continue };
            if apps.iter().any(|a| a.id == id) {
                continue;
            }
            let name = app.localizedName().map_or_else(|| id.0.clone(), |n| n.to_string());
            apps.push(AppInfo { id, name });
        }
        apps.sort_by_key(|a| a.name.to_lowercase());
        apps
    }
}
