# Rauncher
Rauncher is an application launcher for Linux desktop. It supports both X11 and Wayland.

## Install
### AUR

```terminal
$ yay -S rauncher-wayland # or rauncher-x11
```

### Manual
Please install required packages:

- snixembed (X11)
- gtk4-layer-shell (Wayland)


```terminal
$ git clone https://github.com/h3poteto/rauncher.git
$ cd rauncher
$ make build-wayland # or build-x11
$ sudo make install
```

## Usage
### X11
The configuration file will be created after you launch `rauncher`. So, please update the file.

```toml
[hotkey]
key = 102         # You can check the keycode with xev.
modifier = "ctrl" # "ctrl", "shift", or "alt"

[[custom_search]]
name = "Google"
exec = "https://www.google.com/search?q=%q"
icon_name = "web-browser"
default_search = true
```

The default hotkey is <kbd>Ctrk</kbd>+<kbd>Space</kbd>. You can check the keycode using `xev`.

### Wayland
In wayland, `rauncher` can't catch global shortcut keys, so `hotkey` section in the configuration file is ignored. Instead, `rauncher` provides subcommand.
```
$ rauncher toggle
```

So, plsase set a shortcut key in your compositor settings, e.g. sway

```
exec sleep 2 && rauncher

bindsym Control+space exec rauncher toggle
```

## License
Rauncher is licensed under [GPL-3.0](LICENSE).

Copyright (C) 2026 h3poteto
