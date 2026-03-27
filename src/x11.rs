use std::sync::mpsc;

use x11rb::{
    connection::Connection,
    protocol::{
        Event,
        xinerama,
        xproto::{ConfigureWindowAux, ConnectionExt, GrabMode, ModMask},
    },
};

use crate::config;
use crate::events::KeyEvent;

const TOP_MARGIN: i32 = 420;

pub fn bind_shortcut_key(
    sender: mpsc::Sender<KeyEvent>,
    c: &config::Config,
) -> Result<(), Box<dyn std::error::Error>> {
    let (conn, screen_num) = x11rb::connect(None)?;
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;

    let mut modifier = ModMask::CONTROL;
    match c.hotkey.modifier.as_str() {
        "shift" => modifier = ModMask::SHIFT,
        "alt" => modifier = ModMask::M1,
        _ => {}
    }

    conn.grab_key(
        false,
        root,
        modifier,
        c.hotkey.key,
        GrabMode::ASYNC,
        GrabMode::ASYNC,
    )?;
    conn.flush()?;

    loop {
        let event = conn.wait_for_event().expect("Failed to get event");
        match event {
            Event::KeyPress(key) => {
                if key.detail == c.hotkey.key {
                    tracing::debug!("{:#?}", key.detail);
                    tracing::debug!("hotkey pressed");
                    let _ = sender.send(KeyEvent::WindowToggle);
                } else {
                    tracing::debug!("other key events");
                }
            }
            _ => {
                tracing::debug!("other events");
            }
        }
    }
}

pub fn center_on_active_monitor(
    window_xid: u32,
    window_width: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    let (conn, screen_num) = x11rb::connect(None)?;
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;

    let pointer = conn.query_pointer(root)?.reply()?;
    let cursor_x = pointer.root_x as i32;
    let cursor_y = pointer.root_y as i32;

    let (mon_x, mon_y, mon_width) =
        if let Ok(screens) = xinerama::query_screens(&conn)?.reply() {
            screens
                .screen_info
                .iter()
                .find(|s| {
                    let sx = s.x_org as i32;
                    let sy = s.y_org as i32;
                    let sw = s.width as i32;
                    let sh = s.height as i32;
                    cursor_x >= sx && cursor_x < sx + sw && cursor_y >= sy && cursor_y < sy + sh
                })
                .map(|s| (s.x_org as i32, s.y_org as i32, s.width as i32))
                .unwrap_or((0, 0, screen.width_in_pixels as i32))
        } else {
            (0, 0, screen.width_in_pixels as i32)
        };

    let x = mon_x + (mon_width - window_width) / 2;
    let y = mon_y + TOP_MARGIN;

    conn.configure_window(window_xid, &ConfigureWindowAux::new().x(x).y(y))?;
    conn.flush()?;

    tracing::debug!(
        "Centered window on monitor: x={}, y={}, monitor_x={}, monitor_width={}",
        x,
        y,
        mon_x,
        mon_width
    );

    Ok(())
}
