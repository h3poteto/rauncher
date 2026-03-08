#[derive(Debug)]
pub struct RauncherTray {}

impl ksni::Tray for RauncherTray {
    fn id(&self) -> String {
        "rauncher".into()
    }

    // fn icon_name(&self) -> String {
    //     "rauncher".into()
    // }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        let img = image::load_from_memory(include_bytes!(
            "../data/icons/hicolor/128x128/apps/rauncher.png"
        ))
        .unwrap()
        .into_rgba8();

        let (w, h) = img.dimensions();
        vec![ksni::Icon {
            width: w as i32,
            height: h as i32,
            data: img.into_raw(),
        }]
    }

    fn title(&self) -> String {
        "rauncher".into()
    }
}
