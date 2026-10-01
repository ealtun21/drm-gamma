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

> **KDE Plasma and GNOME reset the gamma table** when you switch back, so the
> TTY trick doesn't stick there. On KDE, drm-gamma [handles this automatically](#kde-plasma-automatic);
> on GNOME use an [ICC profile](#icc-profile-only-gnome-others). Other compositors may also reset it on hotplug/DPMS wake; redo the
> TTY round-trip then.

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

## Saturation

Like [nVibrant](https://github.com/Tremeschin/nVibrant) /
NVIDIA's *digital vibrance*, but for any driver exposing the DRM `CTM`
property (amdgpu, i915/xe, …). `-s` / `SATURATION` takes 0–2: `1.0` unchanged, `0` grayscale,
`>1` more vivid. It's set through the same DRM master window as gamma (TTY round-trip), and
on KDE / `--icc` it's baked into the profile's primaries instead (floored at 0.1 there). The
NVIDIA proprietary driver has no `CTM`; use nVibrant there.

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
sudo drm-gamma -s 1.3                                      # 30% more saturation
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

### KDE Plasma (automatic)

KWin overwrites the DRM gamma table, but it applies an ICC profile's `vcgt`
calibration curve and keeps it across reboots. drm-gamma detects a running
KWin and switches to that path by itself. **Run it as your user, not sudo:**

```bash
drm-gamma -rgamma 0.80 -ggamma 0.80 -bgamma 0.90   # applied live to every enabled output
drm-gamma -r                                       # back to KWin's built-in sRGB
```

It writes `~/.local/share/icc/drm-gamma-t6500-br1.00-r0.80-g0.80-b0.90.icc`,
points each output at it via `kscreen-doctor`, and deletes older
`drm-gamma-*.icc` files. Each setting gets its own filename because KWin
caches profiles by path. It works over ssh too; the session's Wayland socket
is found automatically. No daemon needed: `sudo systemctl disable --now drm-gamma`.

### ICC profile only (GNOME, others)

`--icc DIR|FILE` writes the profile without applying it and prints the path.
A directory gets an auto-named file:

```bash
drm-gamma --icc ~/.local/share/icc -rgamma 0.80 -ggamma 0.80 -bgamma 0.90
```

GNOME: Settings → Color → your display → Add profile → import the file.

### CLI

| Flag | Description |
|------|-------------|
| `-rgamma`, `-ggamma`, `-bgamma` (or `--rgamma` …) | Per-channel gamma, 0.1–10 |
| `-t, --temperature K` | Color temperature, 1000–10000 (default 6500) |
| `-b, --brightness X` | Brightness multiplier, 0.1–1.0 |
| `-s, --saturation X` | Saturation, 0–2 (1 = unchanged) |
| `--icc DIR\|FILE` | Write an ICC profile instead of applying via DRM (a directory auto-names it). On KDE, applied automatically |
| `-d, --device PATH` | DRM device (default `/dev/dri/card1`, auto-falls back) |
| `-r, --reset` | Reset to 6500K, brightness 1, gamma 1, saturation 1 |
| `-l, --list` | List DRM devices, CRTCs, connectors |
| `-D, --daemon` | Run as daemon (root) |
| `-c, --config PATH` | Daemon config (default `/etc/default/drm-gamma.conf`) |
| `-v, --verbose` | Debug logging |

## Configuration

`/etc/default/drm-gamma.conf`:

| Option | Default | Description |
|--------|---------|-------------|
| RGAMMA / GGAMMA / BGAMMA | 1.0 | Per-channel gamma (0.1–10) |
| SATURATION | 1.0 | Saturation via CTM (0–2) |
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
