use super::*;

#[derive(Default)]
pub(super) struct WindowState {
    creation: Mutex<()>,
}

pub(super) fn window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    if matches!(event, tauri::WindowEvent::Destroyed) {
        application_menu::clear_window(window.app_handle(), window.label());
    }
    if window.label() == "rendering" {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = window.emit("cuemol-render-close", ());
        }
    }
}

#[tauri::command]
pub(super) async fn render_window_request(
    app: tauri::AppHandle,
    operation: String,
    args: Option<serde_json::Value>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let lifecycle = app.state::<WindowState>();
        let _guard = lifecycle
            .creation
            .lock()
            .map_err(|error| error.to_string())?;
        match operation.as_str() {
            "open" => {
                let model = app.state::<ManagedState>();
                request_model_via(
                    &model.sender,
                    ModelRequest::Rendering {
                        operation: "open".into(),
                        args: args.unwrap_or_else(|| serde_json::json!({})),
                    },
                )?;
                if let Some(window) = app.get_webview_window("rendering") {
                    window
                        .emit("cuemol-render-open", ())
                        .map_err(|error| error.to_string())?;
                } else {
                    tauri::WebviewWindowBuilder::new(
                        &app,
                        "rendering",
                        tauri::WebviewUrl::App("rendering.html".into()),
                    )
                    .title("Rendering")
                    .inner_size(1000., 700.)
                    .min_inner_size(480., 400.)
                    .visible(false)
                    .build()
                    .map_err(|error| error.to_string())?;
                }
            }
            "show" | "hide" => {
                let window = app
                    .get_webview_window("rendering")
                    .ok_or("Rendering window is unavailable")?;
                if operation == "hide" {
                    application_menu::clear_window(&app, "rendering");
                    window.hide().map_err(|error| error.to_string())?;
                } else {
                    window.show().map_err(|error| error.to_string())?;
                    window.unminimize().map_err(|error| error.to_string())?;
                    window.set_focus().map_err(|error| error.to_string())?;
                }
            }
            "main" | "quit" => {
                let window = app
                    .get_webview_window("main")
                    .ok_or("Main window is unavailable")?;
                window.show().map_err(|error| error.to_string())?;
                window.unminimize().map_err(|error| error.to_string())?;
                window.set_focus().map_err(|error| error.to_string())?;
                if operation == "quit" {
                    window
                        .emit("cuemol-request-quit", ())
                        .map_err(|error| error.to_string())?;
                }
            }
            _ => return Err("Unknown Rendering window operation".into()),
        }
        Ok(())
    })
    .await
    .map_err(|error| error.to_string())?
}
