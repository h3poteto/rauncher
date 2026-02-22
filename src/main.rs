use std::sync::mpsc;

use freedesktop_desktop_entry::{DesktopEntry, Iter, default_paths};
use gtk4::{
    Align, Application, ApplicationWindow, Entry, EventControllerKey, Orientation, gdk,
    gio::prelude::{ApplicationExt, ApplicationExtManual},
    glib,
    prelude::{
        BoxExt, EditableExt, EntryExt, GtkApplicationExt, GtkWindowExt, ListBoxRowExt, WidgetExt,
    },
};
use nucleo_matcher::{
    Matcher, Utf32Str,
    pattern::{CaseMatching, Normalization, Pattern},
};
use x11rb::{
    connection::Connection,
    protocol::{
        Event,
        xproto::{ConnectionExt, GrabMode, ModMask},
    },
};

struct Desktop {
    name: String,
    entry: DesktopEntry,
}

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

        let mut desktop_entries = Vec::<Desktop>::new();

        for path in Iter::new(default_paths()) {
            if let Ok(entry) = DesktopEntry::from_path(path, Some(&["en"])) {
                let d = Desktop {
                    name: entry.name(&["en"]).unwrap().to_string(),
                    entry,
                };
                desktop_entries.push(d);
            }
        }

        println!("entries: {}", desktop_entries.len());

        build_ui(app, desktop_entries);
    });

    app.connect_activate(|_app| {});

    app.run();
}

fn build_ui(app: &Application, desktop_entries: Vec<Desktop>) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Rauncher")
        .default_width(480)
        .default_height(-1)
        .decorated(false)
        .modal(true)
        .build();

    let window_clone = window.clone();
    let controller = EventControllerKey::new();
    let list_box = gtk4::ListBox::new();

    let list_box_copy = list_box.clone();
    controller.connect_key_pressed(move |_controller, key, _keycode, _modifier| match key {
        gdk::Key::Escape => {
            window_clone.hide();
            glib::Propagation::Stop
        }
        gdk::Key::Down => {
            let selected = list_box_copy.selected_row();
            let next = match selected {
                Some(row) => list_box_copy.row_at_index(row.index() + 1),
                None => list_box_copy.row_at_index(0),
            };
            if let Some(next_row) = next {
                list_box_copy.select_row(Some(&next_row));
            }
            glib::Propagation::Stop
        }
        gdk::Key::Up => {
            let selected = list_box_copy.selected_row();
            let prev = match selected {
                Some(row) if row.index() > 0 => list_box_copy.row_at_index(row.index() - 1),
                _ => None,
            };
            if let Some(prev_row) = prev {
                list_box_copy.select_row(Some(&prev_row));
            }
            glib::Propagation::Stop
        }
        _ => glib::Propagation::Proceed,
    });

    window.add_controller(controller);

    let css_provider = gtk4::CssProvider::new();
    css_provider.load_from_data(
        "
window { border-radius: 4px; }
entry:focus-within { outline: none; box-shadow: none; border-color: transparent; }
entry { font-size: 24px; padding: 12px; min-height: 48px; }
",
    );

    gtk4::style_context_add_provider_for_display(
        &gdk::Display::default().unwrap(),
        &css_provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let search_entry = Entry::builder().placeholder_text("Search...").build();

    let list_box_clone = list_box.clone();
    let window_clone = window.clone();
    search_entry.connect_changed(move |entry| {
        let text = entry.text().to_string();

        let mut result: Vec<_> = vec![];
        if text.len() > 0 {
            let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
            let pattern = Pattern::parse(text.as_str(), CaseMatching::Ignore, Normalization::Smart);

            result = desktop_entries
                .iter()
                .filter(|d| d.entry.icon().is_some() && d.entry.exec().is_some())
                .filter_map(|d| {
                    let mut buf = Vec::new();
                    let haystack = Utf32Str::new(&d.name, &mut buf);
                    pattern
                        .score(haystack, &mut matcher)
                        .map(|score| (d, score))
                })
                .collect();

            result.sort_by(|a, b| b.1.cmp(&a.1));
        }

        while let Some(child) = list_box_clone.first_child() {
            list_box_clone.remove(&child);
        }

        for (desktop, _score) in result.iter().take(10) {
            let row = gtk4::ListBoxRow::new();
            let hbox = gtk4::Box::new(Orientation::Horizontal, 0);
            hbox.set_margin_start(16);
            hbox.set_margin_end(16);
            hbox.set_margin_top(4);
            hbox.set_margin_bottom(4);

            if let Some(icon_name) = &desktop.entry.icon() {
                let image = gtk4::Image::from_icon_name(icon_name);
                image.set_pixel_size(32);
                hbox.append(&image);
            }

            let vbox = gtk4::Box::new(Orientation::Vertical, 2);
            vbox.set_halign(Align::Start);
            vbox.set_margin_start(8);

            let name_label = gtk4::Label::new(Some(&desktop.name));
            name_label.set_halign(Align::Start);
            vbox.append(&name_label);

            if let Some(comment) = &desktop.entry.comment(&["en"]) {
                let comment_label = gtk4::Label::new(Some(comment));
                comment_label.set_halign(Align::Start);
                comment_label.add_css_class("dim-label");
                vbox.append(&comment_label);
            }

            hbox.append(&vbox);
            row.set_child(Some(&hbox));

            row.set_widget_name(&desktop.entry.exec().unwrap());
            list_box_clone.append(&row);
        }
        window_clone.set_default_height(-1);
    });

    let cloned_list_box = list_box.clone();
    search_entry.connect_activate(move |_e| {
        if let Some(row) = cloned_list_box.selected_row() {
            row.activate();
        }
    });

    let window_copy = window.clone();
    list_box.connect_row_activated(move |_list_box, row| {
        let exec = row.widget_name().to_string();
        std::process::Command::new("sh")
            .arg("-c")
            .arg(&exec)
            .spawn()
            .expect("Failed to execute");
        window_copy.hide();
    });

    let vbox = gtk4::Box::new(Orientation::Vertical, 0);
    vbox.append(&search_entry);
    vbox.append(&list_box);

    window.set_child(Some(&vbox));
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
