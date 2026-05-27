use std::sync::mpsc;

use x11rb::{
    connection::Connection,
    protocol::{
        Event,
        xinerama,
        xproto::{AtomEnum, ChangeWindowAttributesAux, ClientMessageEvent, ConfigureWindowAux, ConnectionExt, EventMask, GrabMode, ModMask, PropMode},
    },
    wrapper::ConnectionExt as _,
};

use crate::config;
use crate::events::KeyEvent;

const TOP_MARGIN: i32 = 420;

// _NET_MOVERESIZE_WINDOW data.l[0] flags
// gravity (bits 0-7) = 0 -> use the gravity from WM_NORMAL_HINTS
// bit 8: x present, bit 9: y present
// bits 12-15: source indication (1 = normal application)
const NET_MOVERESIZE_FLAGS: u32 = (1 << 8) | (1 << 9) | (1 << 12);

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

    let (mon_x, mon_y, mon_width) = pick_monitor(&conn, screen, cursor_x, cursor_y);

    let x = mon_x + (mon_width - window_width) / 2;
    let y = mon_y + TOP_MARGIN;

    // Most modern WMs honour _NET_MOVERESIZE_WINDOW for already-mapped windows
    // even when they ignore plain ConfigureWindow.
    if let Err(e) = send_net_moveresize(&conn, root, window_xid, x, y) {
        tracing::debug!("_NET_MOVERESIZE_WINDOW failed: {}", e);
    }

    // Fallback / extra: plain ConfigureWindow.
    conn.configure_window(window_xid, &ConfigureWindowAux::new().x(x).y(y))?;
    conn.flush()?;

    tracing::debug!(
        "Centered window on monitor: x={}, y={}, cursor=({},{}), monitor=({},{},w={})",
        x,
        y,
        cursor_x,
        cursor_y,
        mon_x,
        mon_y,
        mon_width
    );

    Ok(())
}

fn pick_monitor(
    conn: &impl Connection,
    screen: &x11rb::protocol::xproto::Screen,
    cursor_x: i32,
    cursor_y: i32,
) -> (i32, i32, i32) {
    let root_fallback = (0, 0, screen.width_in_pixels as i32);
    let reply = match xinerama::query_screens(conn) {
        Ok(cookie) => match cookie.reply() {
            Ok(r) => r,
            Err(_) => return root_fallback,
        },
        Err(_) => return root_fallback,
    };
    let infos = &reply.screen_info;
    if infos.is_empty() {
        return root_fallback;
    }

    // 1. Prefer the monitor that contains the cursor.
    if let Some(s) = infos.iter().find(|s| {
        let sx = s.x_org as i32;
        let sy = s.y_org as i32;
        let sw = s.width as i32;
        let sh = s.height as i32;
        cursor_x >= sx && cursor_x < sx + sw && cursor_y >= sy && cursor_y < sy + sh
    }) {
        return (s.x_org as i32, s.y_org as i32, s.width as i32);
    }

    // 2. Otherwise pick the monitor whose center is closest to the cursor —
    //    avoids the previous fallback that used the total root width and
    //    landed on the seam between monitors.
    if let Some(s) = infos.iter().min_by_key(|s| {
        let cx = s.x_org as i32 + s.width as i32 / 2;
        let cy = s.y_org as i32 + s.height as i32 / 2;
        let dx = (cx - cursor_x) as i64;
        let dy = (cy - cursor_y) as i64;
        dx * dx + dy * dy
    }) {
        return (s.x_org as i32, s.y_org as i32, s.width as i32);
    }

    root_fallback
}

// Set _NET_WM_WINDOW_TYPE_DIALOG so that tiling WMs (e.g. i3) treat this
// window as floating automatically, which is required for MOVERESIZE to work.
pub fn set_window_type_dialog(window: u32) -> Result<(), Box<dyn std::error::Error>> {
    let (conn, _) = x11rb::connect(None)?;
    let type_atom = conn
        .intern_atom(false, b"_NET_WM_WINDOW_TYPE")?
        .reply()?
        .atom;
    let dialog_atom = conn
        .intern_atom(false, b"_NET_WM_WINDOW_TYPE_DIALOG")?
        .reply()?
        .atom;
    conn.change_property32(PropMode::REPLACE, window, type_atom, AtomEnum::ATOM, &[dialog_atom])?;
    // Also subscribe to ConfigureNotify so the WM notifies us after placement.
    conn.change_window_attributes(
        window,
        &ChangeWindowAttributesAux::new().event_mask(EventMask::STRUCTURE_NOTIFY),
    )?;
    conn.flush()?;
    Ok(())
}

fn send_net_moveresize(
    conn: &impl Connection,
    root: u32,
    window: u32,
    x: i32,
    y: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    let atom = conn
        .intern_atom(false, b"_NET_MOVERESIZE_WINDOW")?
        .reply()?
        .atom;
    let event = ClientMessageEvent::new(
        32,
        window,
        atom,
        [NET_MOVERESIZE_FLAGS, x as u32, y as u32, 0, 0],
    );
    conn.send_event(
        false,
        root,
        EventMask::SUBSTRUCTURE_NOTIFY | EventMask::SUBSTRUCTURE_REDIRECT,
        event,
    )?;
    Ok(())
}
