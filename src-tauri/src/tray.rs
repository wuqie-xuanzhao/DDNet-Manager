//! 系统托盘图标与托盘菜单事件处理。
//!
//! 托盘行为约定：
//! - 左键单击托盘图标：显示并聚焦主窗口。
//! - 右键单击托盘图标：在图标附近显示独立的托盘菜单窗口（位置按主窗口缩放
//!   因子换算成逻辑坐标）。
//! - `tray-menu-action` 事件：托盘菜单窗口通过 emit 触发，payload 取 `show`
//!   或 `quit`，分别显示主窗口和退出应用。

use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Listener, Manager, PhysicalPosition};

/// 构建系统托盘图标，并注册托盘事件与菜单 action 监听器。
///
/// tray id 固定为 `MAIN_TRAY_ID`，便于 main.rs 在 CloseRequested 时显式 destroy
/// （review issue #5：避免托盘图标依赖 OS 进程退出回收）。
pub const MAIN_TRAY_ID: &str = "ddnet-manager-main-tray";

pub fn setup_tray(app: &tauri::App) -> Result<(), tauri::Error> {
    let mut tray_builder = TrayIconBuilder::with_id(MAIN_TRAY_ID).tooltip("DDNet Manager");
    if let Some(icon) = app.default_window_icon() {
        tray_builder = tray_builder.icon(icon.clone());
    }
    tray_builder
        .on_tray_icon_event(handle_tray_icon_event)
        .build(app)?;

    let app_handle = app.handle().clone();
    app.listen("tray-menu-action", move |event| {
        handle_tray_menu_action(&app_handle, event.payload());
    });

    Ok(())
}

/// 处理托盘图标点击事件。
fn handle_tray_icon_event(tray: &TrayIcon, event: TrayIconEvent) {
    match event {
        TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } => show_main_window(tray.app_handle()),
        TrayIconEvent::Click {
            button: MouseButton::Right,
            button_state: MouseButtonState::Up,
            position,
            ..
        } => position_tray_menu(tray.app_handle(), position),
        _ => {}
    }
}

/// 显示并聚焦主窗口。
fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// 按托盘图标的物理坐标定位托盘菜单窗口，并显示。
fn position_tray_menu(app: &AppHandle, position: PhysicalPosition<f64>) {
    let (Some(tray_menu), Some(main)) = (
        app.get_webview_window("tray-menu"),
        app.get_webview_window("main"),
    ) else {
        return;
    };
    // 根据主窗口缩放因子，将托盘图标物理坐标转为逻辑坐标来定位菜单。
    let Ok(scale) = main.scale_factor() else {
        return;
    };
    let logical: tauri::LogicalPosition<f64> = position.to_logical(scale);
    let _ = tray_menu.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(
        logical.x - 60.0,
        logical.y - 130.0,
    )));
    let _ = tray_menu.show();
    let _ = tray_menu.set_focus();
}

/// 处理来自托盘菜单窗口的 action 事件。
///
/// payload 可能为 JSON 序列化的 `"show"`（含引号）或裸字符串 `show`，
/// 用 `trim_matches('"')` 兼容两种形态。
fn handle_tray_menu_action(app: &AppHandle, payload: &str) {
    match payload.trim_matches('"') {
        "show" => show_main_window(app),
        "quit" => app.exit(0),
        _ => {}
    }
}
