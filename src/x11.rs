use std::sync::mpsc;

use x11rb::{
    connection::Connection,
    protocol::{
        Event,
        xproto::{ConnectionExt, GrabMode, ModMask},
    },
};

use crate::config;
use crate::events::KeyEvent;

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
