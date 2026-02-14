use std::sync::mpsc;

use gtk4::{
    Application, ApplicationWindow, EventControllerKey, gdk,
    gio::prelude::{ApplicationExt, ApplicationExtManual},
    glib,
    prelude::{GtkApplicationExt, GtkWindowExt, WidgetExt},
};
use x11rb::{
    connection::Connection,
    protocol::{
        Event,
        xproto::{ConnectionExt, GrabMode, ModMask},
    },
};

fn main() {
    let app = Application::builder()
        .application_id("dev.h3poteto.rauncher")
        .build();

    app.connect_startup(move |app| {
        let (key_sender, key_receiver) = mpsc::channel::<KeyEvent>();
        let app_clone = app.clone();
        glib::idle_add_local(move || {
            let windows = app_clone.windows();
            if let Ok(msg) = key_receiver.try_recv() {
                match msg {
                    KeyEvent::WindowToggle => {
                        windows.iter().for_each(|w| {
                            if w.is_visible() {
                                w.hide();
                            } else {
                                w.present();
                            }
                        });
                    }
                }
            }
            glib::ControlFlow::Continue
        });

        let key_sender = key_sender.clone();
        std::thread::spawn(move || {
            if let Err(err) = bind_shortcut_key(key_sender) {
                println!("{}", err);
                std::process::exit(1);
            }
        });
    });

    app.connect_activate(|app| {
        build_ui(app);
    });

    app.run();
}

fn build_ui(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Rauncher")
        .default_width(480)
        .default_height(120)
        .decorated(false)
        .modal(true)
        .build();

    let window_clone = window.clone();
    let controller = EventControllerKey::new();
    controller.connect_key_pressed(move |_controller, key, _keycode, _modifier| {
        if key == gdk::Key::Escape {
            window_clone.hide();
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });

    window.add_controller(controller);
    window.hide();
}

fn bind_shortcut_key(sender: mpsc::Sender<KeyEvent>) -> Result<(), Box<dyn std::error::Error>> {
    let (conn, screen_num) = x11rb::connect(None)?;
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;

    let keycode = 102;

    conn.grab_key(
        false,
        root,
        ModMask::CONTROL,
        keycode,
        GrabMode::ASYNC,
        GrabMode::ASYNC,
    )?;
    conn.flush()?;

    loop {
        let event = conn.wait_for_event().expect("Failed to get event");
        match event {
            Event::KeyPress(key) => {
                if key.detail == keycode {
                    println!("{:#?}", key.detail);
                    println!("hotkey pressed");
                    let _ = sender.send(KeyEvent::WindowToggle);
                } else {
                    println!("other key events");
                }
            }
            _ => {
                println!("other events");
            }
        }
    }
}

enum KeyEvent {
    WindowToggle,
}
