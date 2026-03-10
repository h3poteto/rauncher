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

## License
Rauncher is licensed under [GPL-3.0](LICENSE).

Copyright (C) 2026 h3poteto
