use std::{
    io::Write,
    time::{SystemTime, UNIX_EPOCH},
};

use tauri::{PhysicalPosition, PhysicalSize, Runtime, Size, WebviewWindow};

use crate::overview::nexus_paths_for_home;

pub(crate) const MAIN_WINDOW_LABEL: &str = "main";
pub(crate) const DESKTOP_RUNTIME_MARKER_SCRIPT: &str = r#"
window.__NEXUSHUB_DESKTOP_RUNTIME__ = true;
"#;

const DESKTOP_BOOT_PROBE_SCRIPT: &str = r#"
(function () {
  var root = document.getElementById("root");
  var bodyText = (document.body && document.body.innerText ? document.body.innerText : "").replace(/\s+/g, " ").trim();
  return {
    readyState: document.readyState,
    title: document.title,
    desktopRuntime: window.__NEXUSHUB_DESKTOP_RUNTIME__ === true,
    bootMounted: Boolean(window.__NEXUSHUB_BOOT__ && window.__NEXUSHUB_BOOT__.mounted),
    rootChildren: root ? root.children.length : -1,
    rootClass: root && root.firstElementChild ? root.firstElementChild.className : "",
    bodyTextLength: bodyText.length,
    hasMainShell: Boolean(document.querySelector(".app-shell")),
    hasDesktopNav: bodyText.indexOf("Codex") >= 0 && bodyText.indexOf("探针") >= 0 && bodyText.indexOf("运维") >= 0,
    hasWebLoginGate: Boolean(document.querySelector(".login-shell")),
    hasVisibleLinuxHostCopy: Boolean(document.querySelector(".security-workspace, .turnstile-box"))
  };
})()
"#;

pub(crate) fn reveal_main_window<R: Runtime>(window: &WebviewWindow<R>) {
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.maximize();
    fit_main_window_to_work_area(window);
    let _ = window.set_focus();
}

fn fit_main_window_to_work_area<R: Runtime>(window: &WebviewWindow<R>) {
    let monitor = match window.current_monitor() {
        Ok(Some(monitor)) => Some(monitor),
        _ => window.primary_monitor().ok().flatten(),
    };
    let Some(monitor) = monitor else {
        return;
    };
    let work_area = monitor.work_area();
    if work_area.size.width == 0 || work_area.size.height == 0 {
        return;
    }
    let _ = window.set_position(PhysicalPosition::new(
        work_area.position.x,
        work_area.position.y,
    ));
    let _ = window.set_size(Size::Physical(PhysicalSize::new(
        work_area.size.width,
        work_area.size.height,
    )));
}

pub(crate) fn schedule_delayed_main_window_reveal<R: Runtime>(window: &WebviewWindow<R>) {
    let delayed_window = window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(250));
        let main_thread_window = delayed_window.clone();
        let _ = delayed_window.run_on_main_thread(move || {
            reveal_main_window(&main_thread_window);
        });
    });
}

fn append_desktop_app_log(message: &str) {
    let Some(home) = dirs::home_dir() else {
        return;
    };
    let paths = nexus_paths_for_home(home);
    if std::fs::create_dir_all(&paths.log_dir).is_err() {
        return;
    }
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths.app_log_file)
    else {
        return;
    };
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let _ = writeln!(file, "{timestamp} {message}");
}

pub(crate) fn schedule_desktop_boot_probe<R: tauri::Runtime>(window: &WebviewWindow<R>) {
    let probe_window = window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(2));
        if let Err(err) = probe_window.eval_with_callback(DESKTOP_BOOT_PROBE_SCRIPT, |payload| {
            append_desktop_app_log(&format!("desktop_boot_probe {payload}"));
        }) {
            append_desktop_app_log(&format!("desktop_boot_probe_error {err}"));
        }
    });
}
