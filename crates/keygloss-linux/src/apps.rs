//! `ForegroundApp` on KDE Plasma 6: Wayland gives clients no way to see other windows, so a
//! small KWin script reports every window activation to us over D-Bus, with the apps that have
//! a taskbar window. Apps are identified by their desktop file name (`org.kde.konsole`), falling
//! back to the window class. Other desktops report no foreground app.

use std::sync::{Arc, Mutex, OnceLock};

use keygloss_core::platform::{AppId, AppInfo, ForegroundApp};

const PATH: &str = "/com/keygloss/Focus";
const PLUGIN: &str = "keygloss-focus";

/// Calls `Activated(app, apps)` on every activation; `apps` is one app id per line.
const SCRIPT: &str = r#"
function id(w) { return String(w.desktopFileName || w.resourceClass || ""); }
function send() {
    const apps = workspace.windowList().filter(w => w.normalWindow && !w.skipTaskbar).map(id);
    const w = workspace.activeWindow;
    callDBus("@BUS@", "@PATH@", "com.keygloss.Focus", "Activated", w ? id(w) : "", apps.join("\n"));
}
workspace.windowActivated.connect(send);
send();
"#;

type Callback = Arc<dyn Fn(AppId) + Send + Sync>;

#[derive(Default)]
struct State {
    current: Option<AppId>,
    /// Desktop file names or window classes, as KWin reports them.
    apps: Vec<String>,
    callbacks: Vec<Callback>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
static CONNECTION: OnceLock<Option<zbus::blocking::Connection>> = OnceLock::new();

fn state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    let mut s = STATE.lock().unwrap_or_else(|p| p.into_inner());
    f(s.get_or_insert_with(State::default))
}

struct Focus;

#[zbus::interface(name = "com.keygloss.Focus")]
impl Focus {
    #[zbus(name = "Activated")]
    fn activated(&self, app: String, apps: String) {
        let app = (!app.is_empty()).then(|| AppId::from_exe(&app));
        let mut list: Vec<String> = apps.lines().filter(|a| !a.is_empty()).map(String::from).collect();
        list.sort();
        list.dedup();
        let changed = state(|s| {
            s.apps = list;
            let changed = app.is_some() && app != s.current;
            if changed {
                s.current = app.clone();
            }
            changed.then(|| s.callbacks.clone())
        });
        // Outside the lock: a callback may ask for the foreground app.
        if let (Some(callbacks), Some(app)) = (changed, app) {
            for callback in callbacks {
                callback(app.clone());
            }
        }
    }
}

/// Serves [`Focus`] and loads the KWin script (once). `None` off KDE Plasma.
fn connect() -> Option<zbus::blocking::Connection> {
    let conn = zbus::blocking::connection::Builder::session().ok()?.serve_at(PATH, Focus).ok()?.build().ok()?;
    let bus = conn.unique_name()?.to_string();
    let dir = std::env::var_os("XDG_RUNTIME_DIR").map_or_else(std::env::temp_dir, std::path::PathBuf::from);
    let file = dir.join(format!("{PLUGIN}.js"));
    std::fs::write(&file, SCRIPT.replace("@BUS@", &bus).replace("@PATH@", PATH)).ok()?;
    let scripting = zbus::blocking::Proxy::new(&conn, "org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting").ok()?;
    // A script left by an earlier run reports to a bus name that is gone.
    let _: Result<bool, _> = scripting.call("unloadScript", &(PLUGIN,));
    let id: i32 = scripting.call("loadScript", &(file.to_string_lossy().as_ref(), PLUGIN)).ok()?;
    let script = zbus::blocking::Proxy::new(&conn, "org.kde.KWin", format!("/Scripting/Script{id}"), "org.kde.kwin.Script").ok()?;
    script.call::<_, _, ()>("run", &()).ok()?;
    Some(conn)
}

fn ensure_connected() {
    CONNECTION.get_or_init(connect);
}

/// Display name from the app's desktop file, if there is one.
fn desktop_name(id: &str) -> Option<String> {
    let data_dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    let home = std::env::var("XDG_DATA_HOME")
        .ok()
        .or_else(|| std::env::var("HOME").ok().map(|h| format!("{h}/.local/share")));
    home.into_iter().chain(data_dirs.split(':').map(String::from)).find_map(|dir| {
        let text = std::fs::read_to_string(format!("{dir}/applications/{id}.desktop")).ok()?;
        text.lines().find_map(|l| l.strip_prefix("Name=")).map(String::from)
    })
}

/// `ForegroundApp` implementation.
#[derive(Clone, Copy, Debug, Default)]
pub struct LinuxForegroundApp;

impl ForegroundApp for LinuxForegroundApp {
    fn foreground_app(&self) -> Option<AppId> {
        ensure_connected();
        state(|s| s.current.clone())
    }

    /// Called from the D-Bus thread; consecutive activations of one app are not reported.
    fn on_foreground_change(&self, callback: Box<dyn Fn(AppId) + Send + Sync>) {
        state(|s| s.callbacks.push(Arc::from(callback)));
        ensure_connected();
    }

    fn running_apps(&self) -> Vec<AppInfo> {
        ensure_connected();
        let mut apps: Vec<AppInfo> = state(|s| s.apps.clone())
            .into_iter()
            .map(|raw| AppInfo { name: desktop_name(&raw).unwrap_or_else(|| raw.clone()), id: AppId::from_exe(&raw) })
            .collect();
        apps.sort_by_key(|a| a.name.to_lowercase());
        apps.dedup_by(|a, b| a.id == b.id);
        apps
    }
}
