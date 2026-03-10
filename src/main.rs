use std::sync::mpsc;

use events::KeyEvent;
use freedesktop_desktop_entry::{DesktopEntry, Iter, default_paths};
use gtk4::{
    Align, Application, ApplicationWindow, Entry, EventControllerKey, IconTheme, Orientation,
    Window, gdk,
    gio::prelude::{ApplicationExt, ApplicationExtManual},
    glib::{self},
    pango,
    prelude::{
        BoxExt, EditableExt, EntryExt, GtkApplicationExt, GtkWindowExt, ListBoxRowExt, WidgetExt,
    },
};
use ksni::blocking::TrayMethods;
use nucleo_matcher::{
    Matcher, Utf32Str,
    pattern::{CaseMatching, Normalization, Pattern},
};
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use zbus::{
    blocking::{Connection, connection},
    interface,
};

mod config;
mod error;
mod events;
mod tray;
#[cfg(feature = "x11")]
mod x11;

struct Desktop {
    name: String,
    entry: DesktopEntry,
}

fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(|s| s.as_str()) == Some("toggle") {
        tracing::debug!("toggle");
        let conn = Connection::session().expect("failed to get connection");
        let reply = conn.call_method(
            Some("com.github.h3poteto.rauncher"),
            "/com/github/h3poteto/rauncher",
            Some("com.github.h3poteto.rauncher"),
            "Toggle",
            &(),
        );
        if let Err(err) = reply {
            tracing::error!("{}", err);
        }
        return;
    }

    let config_dir = dirs::config_dir()
        .expect("config directory not found")
        .join("rauncher");
    let config_file = "config.toml";
    let config_path = config_dir.join(config_file);

    let Some(c) = config::parse_config(&config_path).ok().or_else(|| {
        config::write_default_config(&config_dir, &config_path)
            .map_err(|e| tracing::error!("{}", e))
            .ok()
    }) else {
        tracing::error!("Config file not found");
        return;
    };

    let app = Application::builder()
        .application_id("dev.h3poteto.rauncher")
        .build();

    app.connect_startup(move |app| {
        let exec_path = std::env::current_dir().unwrap();
        let icon_path = exec_path.join("data/icons");
        let icon_theme = IconTheme::for_display(&gdk::Display::default().unwrap());
        icon_theme.add_search_path(icon_path);
        Window::set_default_icon_name("rauncher");

        let (key_sender, key_receiver) = mpsc::channel::<KeyEvent>();

        let key_sender_clone = key_sender.clone();
        let config_clone = c.clone();
        std::thread::spawn(move || {
            if let Err(err) = bind_shortcut_key(key_sender_clone, &config_clone) {
                tracing::error!("{}", err);
                std::process::exit(1);
            }
        });

        let key_sender = key_sender.clone();
        std::thread::spawn(move || {
            let service = RauncherService { sender: key_sender };
            let _conn = connection::Builder::session()
                .expect("Failed to get session")
                .name("com.github.h3poteto.rauncher")
                .expect("Failed to set name")
                .serve_at("/com/github/h3poteto/rauncher", service)
                .expect("Failed to set dbus server")
                .build()
                .expect("Failed to build connection");
            loop {
                std::thread::park();
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

        tracing::debug!("entries: {}", desktop_entries.len());

        let c = c.clone();
        let search_entry = build_ui(app, desktop_entries, &c);

        let app_clone = app.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
            let windows = app_clone.windows();
            if let Ok(msg) = key_receiver.try_recv() {
                match msg {
                    KeyEvent::WindowToggle => {
                        windows.iter().for_each(|w| {
                            if w.is_visible() {
                                search_entry.set_text("");
                                w.hide();
                            } else {
                                w.present();
                                w.set_default_width(480);
                                search_entry.grab_focus();
                            }
                        });
                    }
                }
            }
            glib::ControlFlow::Continue
        });
    });

    app.connect_activate(|_app| {});

    let tray_icon = tray::RauncherTray {};
    let _handle = tray_icon.spawn().unwrap();

    app.run();
}

fn build_ui(app: &Application, desktop_entries: Vec<Desktop>, c: &config::Config) -> Entry {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Rauncher")
        .default_width(480)
        .default_height(-1)
        .decorated(false)
        .modal(true)
        .build();

    #[cfg(feature = "wayland")]
    {
        use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        window.set_keyboard_mode(KeyboardMode::Exclusive);

        window.set_anchor(Edge::Top, true);
        window.set_margin(Edge::Top, 420);
    }

    let window_clone = window.clone();
    let controller = EventControllerKey::new();

    let search_entry = Entry::builder().placeholder_text("Search...").build();
    let list_box = gtk4::ListBox::new();

    let list_box_copy = list_box.clone();

    let search_entry_clone = search_entry.clone();
    controller.connect_key_pressed(move |_controller, key, _keycode, _modifier| match key {
        gdk::Key::Escape => {
            search_entry_clone.set_text("");
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
window { border-radius: 4px; background-color: #484848; outline: none; border-color: #080808; }
entry:focus-within { outline: none; box-shadow: none; border-color: transparent; }
entry { font-size: 24px; padding: 12px; min-height: 48px; background-color: #484848; color: #ededed; outline: none; box-shadow: none; border-color: transparent; }
listbox, row, label, box { background-color: #484848; color: #ededed; }
row:selected, row:selected label, row:selected box, row:selected image, row:focus, row:focus label, row:focus box, row:focus image { background-color: #626262; }
",
    );

    gtk4::style_context_add_provider_for_display(
        &gdk::Display::default().unwrap(),
        &css_provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let list_box_clone = list_box.clone();
    let window_clone = window.clone();
    let c = c.clone();
    search_entry.connect_changed(move |entry| {
        let text = entry.text().to_string();

        let mut result: Vec<_> = vec![];
        if text.len() > 0 {
            let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
            let pattern = Pattern::parse(text.as_str(), CaseMatching::Ignore, Normalization::Smart);

            result = desktop_entries
                .iter()
                .filter(|d| {
                    d.entry.icon().is_some() && d.entry.exec().is_some() && !d.entry.no_display()
                })
                .filter_map(|d| {
                    let mut buf = Vec::new();
                    let mut desc = format!("{}", &d.name);
                    if let Some(exec) = d.entry.exec() {
                        desc = format!("{} {}", desc, exec);
                    }
                    let haystack = Utf32Str::new(&desc, &mut buf);
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
            name_label.set_ellipsize(pango::EllipsizeMode::End);
            vbox.append(&name_label);

            if let Some(comment) = &desktop.entry.comment(&["en"]) {
                let comment_label = gtk4::Label::new(Some(comment));
                comment_label.set_halign(Align::Start);
                comment_label.add_css_class("dim-label");
                comment_label.set_ellipsize(pango::EllipsizeMode::End);
                vbox.append(&comment_label);
            }

            hbox.append(&vbox);
            row.set_child(Some(&hbox));

            row.set_widget_name(&desktop.entry.exec().unwrap());
            list_box_clone.append(&row);
        }

        if text.len() > 0 && result.len() < 10 && c.custom_search.len() > 0 {
            let default_search = c.custom_search.iter().find(|s| s.default_search);
            if let Some(ds) = default_search {
                let row = gtk4::ListBoxRow::new();
                let hbox = gtk4::Box::new(Orientation::Horizontal, 0);
                hbox.set_margin_start(16);
                hbox.set_margin_end(16);
                hbox.set_margin_top(4);
                hbox.set_margin_bottom(4);

                if let Some(icon_name) = &ds.icon_name {
                    let image = gtk4::Image::from_icon_name(icon_name.as_str());
                    image.set_pixel_size(32);
                    hbox.append(&image);
                }
                if let Some(icon_path) = &ds.icon_path {
                    let image = gtk4::Image::from_file(icon_path);
                    image.set_pixel_size(32);
                    hbox.append(&image);
                }

                let vbox = gtk4::Box::new(Orientation::Vertical, 2);
                vbox.set_halign(Align::Start);
                vbox.set_margin_start(8);

                let name_label = gtk4::Label::new(Some("Web search"));
                name_label.set_halign(Align::Start);
                vbox.append(&name_label);

                let comment_label = gtk4::Label::new(Some("Type in your query"));
                comment_label.set_halign(Align::Start);
                comment_label.add_css_class("dim-label");
                vbox.append(&comment_label);

                hbox.append(&vbox);
                row.set_child(Some(&hbox));

                row.set_widget_name(&format!("__web_search__{}__{}", &ds.exec, &text));
                list_box_clone.append(&row);
            }
        }

        if let Some(first_row) = list_box_clone.row_at_index(0) {
            list_box_clone.select_row(Some(&first_row));
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
    let search_entry_copy = search_entry.clone();
    list_box.connect_row_activated(move |_list_box, row| {
        let binding = row.widget_name().to_string();
        if binding.starts_with("__web_search__") {
            let exec = binding.replace("__web_search__", "");
            let v: Vec<&str> = exec.split("__").collect();
            if v.len() < 2 {
                tracing::error!("failed to parse custom search");
                return;
            }
            let url = v[0];
            let argument = v[1];
            let command = url.replace("%q", argument);
            std::process::Command::new("sh")
                .arg("-c")
                .arg(format!(
                    "nohup xdg-open {} >/dev/null 2>&1 &",
                    command.trim()
                ))
                .spawn()
                .expect("Failed to execute");
        } else {
            let exec = binding
                .split_whitespace()
                .filter(|s| !s.starts_with("%"))
                .collect::<Vec<_>>()
                .join(" ");
            std::process::Command::new("sh")
                .arg("-c")
                .arg(format!("nohup {} >/dev/null 2>&1 &", exec.trim()))
                .spawn()
                .expect("Failed to execute");
        }
        window_copy.hide();
        search_entry_copy.set_text("");
    });

    let vbox = gtk4::Box::new(Orientation::Vertical, 0);
    vbox.append(&search_entry);
    vbox.append(&list_box);

    window.set_child(Some(&vbox));
    window.hide();

    search_entry
}

fn bind_shortcut_key(
    sender: mpsc::Sender<KeyEvent>,
    c: &config::Config,
) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "x11")]
    {
        return x11::bind_shortcut_key(sender, c);
    }
    #[cfg(not(feature = "x11"))]
    {
        let _ = (sender, c);
        Ok(())
    }
}

struct RauncherService {
    sender: mpsc::Sender<KeyEvent>,
}

#[interface(name = "com.github.h3poteto.rauncher")]
impl RauncherService {
    async fn toggle(&self) {
        let _ = self.sender.send(KeyEvent::WindowToggle);
    }
}
