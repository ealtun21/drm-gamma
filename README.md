# drm-gamma

[![Build](https://github.com/ealtun21/drm-gamma/actions/workflows/build.yml/badge.svg)](https://github.com/ealtun21/drm-gamma/actions)

Persistent per-channel gamma calibration and color temperature for Linux,
set directly on the GPU's DRM gamma tables. It works the same under Xorg and
any Wayland compositor (COSMIC, KDE, GNOME, Sway, Hyprland, niri, …) because
it never talks to the display server.

```bash
sudo drm-gamma -rgamma 0.80 -ggamma 0.80 -bgamma 0.90
```

Forked from [jjo/drm-colortemp](https://github.com/jjo/drm-colortemp) (a COSMIC
night-light workaround). This fork adds per-channel gamma and drops the COSMIC
applet and legacy C code.

## How it works

A running compositor holds *DRM master*, which stops other processes from
writing the gamma tables. When you switch to a text console it lets go
briefly, and drm-gamma writes the tables then. Most compositors don't rewrite
them when you switch back, so the setting sticks.

The daemon makes this persistent. It watches for TTY switches and
**re-applies your gamma + temperature every time**, so after a reboot or a
compositor reset, one quick TTY round-trip restores the calibration:

1. Press **Ctrl+Alt+F3**. The daemon applies your settings.
2. Press **Ctrl+Alt+F2** (or whichever VT your session is on) to go back.

| TTY | Effect |
|-----|--------|
| F3 (`MONITOR_TTY`) | Time-based temperature (day/night) + gamma |
| F4 (`WARM_TTY`) | Force `NIGHT_TEMP` + gamma |
| F5 (`COOL_TTY`) | Force `DAY_TEMP` + gamma |

> If your compositor resets gamma on its own (e.g. its built-in night light,
> or after a monitor hotplug/DPMS wake), turn that feature off or redo the TTY
> round-trip.

## Install

Requires Rust ≥ 1.70. No libdrm needed (raw ioctls).

```bash
git clone https://github.com/ealtun21/drm-gamma.git
cd drm-gamma
sudo make install            # binary → /usr/local/bin, config → /etc/default/drm-gamma.conf
sudo systemctl enable --now drm-gamma
```

Optional: `sudo make install-notifier` for sunset/sunrise reminders (see
[NOTIFICATIONS.md](NOTIFICATIONS.md)). `make deb VERSION=x.y.z` builds a
Debian package. `sudo make uninstall` removes everything except your config.

## Calibrating gamma

Gamma follows the `xgamma` convention: `out = in^(1/γ)`.

- `1.0` leaves the channel as it is
- `< 1.0` darkens the midtones
- `> 1.0` brightens the midtones
- Black and white endpoints never move. Range is `0.1`–`10`.

Try values from a TTY (or any time the compositor isn't holding master):

```bash
sudo drm-gamma -rgamma 0.80 -ggamma 0.80 -bgamma 0.90      # gamma only (6500K)
sudo drm-gamma -t 4500 -rgamma 0.9 -ggamma 0.9 -bgamma 1.0 # with temperature
sudo drm-gamma -b 0.8 -rgamma 0.9                          # with brightness
sudo drm-gamma -r                                          # reset to linear
sudo drm-gamma -l                                          # list cards/CRTCs/connectors
```

Once you're happy, make it permanent in `/etc/default/drm-gamma.conf`. The
daemon reloads the file on change; no restart needed:

```ini
RGAMMA=0.80
GGAMMA=0.80
BGAMMA=0.90
```

### CLI

| Flag | Description |
|------|-------------|
| `-rgamma`, `-ggamma`, `-bgamma` (or `--rgamma` …) | Per-channel gamma, 0.1–10 |
| `-t, --temperature K` | Color temperature, 1000–10000 (default 6500) |
| `-b, --brightness X` | Brightness multiplier, 0.1–1.0 |
| `-d, --device PATH` | DRM device (default `/dev/dri/card1`, auto-falls back) |
| `-r, --reset` | Reset to 6500K, brightness 1, gamma 1 |
| `-l, --list` | List DRM devices, CRTCs, connectors |
| `-D, --daemon` | Run as daemon (root) |
| `-c, --config PATH` | Daemon config (default `/etc/default/drm-gamma.conf`) |
| `-v, --verbose` | Debug logging |

## Configuration

`/etc/default/drm-gamma.conf`:

| Option | Default | Description |
|--------|---------|-------------|
| RGAMMA / GGAMMA / BGAMMA | 1.0 | Per-channel gamma (0.1–10) |
| DAY_TEMP | 6500 | Daytime temperature (K). Keep at 6500 for gamma-only use |
| NIGHT_TEMP | 3500 | Nighttime temperature (K). Set to 6500 to disable night shift |
| SUNSET_HOUR / SUNRISE_HOUR | 20 / 8 | Day/night switch hours (24h) |
| MONITOR_TTY / WARM_TTY / COOL_TTY | 3 / 4 / 5 | Trigger TTYs |
| DEVICE, DEVICE1…DEVICE8 | *(auto)* | DRM card(s). Omit to auto-detect all |
| CONNECTOR | "" | Limit to one output, e.g. `DP-1`, `HDMI-A-1`, `eDP-1` |
| GAMMA_SIZE | 0 | LUT size override (0 = hardware size) |
| CHECK_INTERVAL | 1 | Poll interval, seconds |
| VERBOSE | 0 | Verbose logging |
| NOTIFY_ENABLED / NOTIFY_USER / NOTIFY_MINUTES_BEFORE | 0 / "" / 5 | Notifier settings |

## Troubleshooting

- **Logs:** `sudo journalctl -u drm-gamma -f`
- **Nothing changes:** you're probably running it while the compositor holds
  master. Do it from a TTY, or use the daemon + TTY round-trip.
- **Wrong card:** run `drm-gamma -l` and set `DEVICE1=` / `CONNECTOR=`.
- **Notifications:** see [NOTIFICATIONS.md](NOTIFICATIONS.md).

## License

Apache License 2.0, see [LICENSE](LICENSE). Original work © jjo
([drm-colortemp](https://github.com/jjo/drm-colortemp)). Color temperature
curve from [Tanner Helland](http://www.tannerhelland.com/4435/convert-temperature-rgb-algorithm-code/).
