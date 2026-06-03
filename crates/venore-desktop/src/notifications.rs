//! App notification emission — sends toasts to the frontend via Tauri events.

use serde::Serialize;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppNotification {
    pub level: String,
    pub title: String,
    pub description: Option<String>,
    pub code: Option<String>,
}

pub fn emit_error(
    app: &AppHandle,
    title: impl Into<String>,
    desc: impl Into<String>,
    code: Option<&str>,
) {
    let n = AppNotification {
        level: "error".into(),
        title: title.into(),
        description: Some(desc.into()),
        code: code.map(|c| c.into()),
    };
    let _ = app.emit("app:notification", &n);
}

#[allow(dead_code)]
pub fn emit_warning(
    app: &AppHandle,
    title: impl Into<String>,
    desc: impl Into<String>,
) {
    let n = AppNotification {
        level: "warning".into(),
        title: title.into(),
        description: Some(desc.into()),
        code: None,
    };
    let _ = app.emit("app:notification", &n);
}
