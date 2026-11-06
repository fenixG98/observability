# guaitop

A lightweight resource monitor for the terminal, for Linux and macOS.

`guaitop` (from the Catalan *guaitar*, "to keep watch") shows CPU, memory, GPU,
temperatures, network, disk and processes in real time, with circular charts, history
graphs with scaled axes, tmux-style navigation and k9s-style `:` commands. It is built to stay out of the way: it sleeps until there is a new
sample or a key press, and it reads `/proc` directly on Linux.

## Features

- **Dashboard**: CPU, memory, GPU and disk donuts; CPU/GPU history with per-core bars;
  temperatures; mirrored network graph; disk I/O.
- **Processes**: sortable and filterable table, a "top CPU users" pie and a detail panel
  with a CPU history for the selected process. Processes can be killed (with confirmation).
- **GPU support**: NVIDIA (NVML), Intel (i915 PMU), AMD and Apple via sysinfo, plus
  per-process GPU usage from DRM `fdinfo` on Linux.
- **Pixel-accurate donuts** on terminals with the Kitty or Sixel graphics protocol
  (for example Kitty or Ghostty); text-block charts everywhere else.
- **Compact view** for small terminals (below 100×30).

## Install

guaitop is built from source with Cargo. There are no prebuilt binaries yet.

### 1. Install Rust (1.88 or newer) and a C toolchain

Linux (Debian/Ubuntu shown; on Arch use `sudo pacman -S base-devel curl git`):

```sh
sudo apt install build-essential curl git
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

macOS:

```sh
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

If Rust is already installed, `rustup update` brings it up to date.

### 2. Install guaitop

Straight from GitHub:

```sh
cargo install --git https://github.com/fenixG98/observability --locked
```

or from a clone:

```sh
git clone https://github.com/fenixG98/observability
cd observability
cargo install --path . --locked    # or: bash bin/deploy/prod.sh
```

Either way the binary ends up in `~/.cargo/bin/guaitop` (make sure `~/.cargo/bin` is on
your `PATH`). To update, run the same command again with `--force`; to remove it,
`cargo uninstall guaitop`.

### Intel GPU usage (Linux)

Reading Intel GPU usage needs the `CAP_PERFMON` capability on the binary:

```sh
sudo setcap cap_perfmon=ep "$(command -v guaitop)"
```

The capability is lost whenever the binary is rebuilt or reinstalled; the TUI shows the
exact command when it is missing.

## Usage

```sh
guaitop            # open the interface
guaitop --probe    # print a one-shot diagnostic (CPU, memory, GPU, temperatures...) and exit
guaitop --help
```

| Key | Action |
|---|---|
| `1` / `2` | Dashboard / Processes tab |
| `n` / `p` | next / previous tab |
| `h` `j` `k` `l`, arrows | move focus between panels |
| `z` | zoom the active panel |
| `g` | next GPU |
| `j` / `k` (processes) | move the selection |
| `x` | kill the selected process |
| `/` | filter processes |
| `:` | command mode (Tab completes) |
| `?` | help |
| `q`, `ctrl+c` | quit |

| Command | Action |
|---|---|
| `:dash`, `:proc` | switch tab |
| `:sort cpu\|mem\|gpu\|pid\|name` | sort processes |
| `:filter <text>` | filter (no text clears it) |
| `:kill <pid>` | kill a process (asks first) |
| `:refresh 500ms` | refresh interval |
| `:gpu <n>`, `:net <iface\|auto>` | pick GPU / network interface |

## Configuration

Copy [`config/config.example.toml`](config/config.example.toml) to
`~/.config/guaitop/config.toml` (Linux) or
`~/Library/Application Support/guaitop/config.toml` (macOS). Every key is optional:
refresh interval, history length, network interface, key bindings, an optional tmux-style
prefix key, truecolor and temperature thresholds.

## Development

```sh
bash bin/test.sh          # fmt, clippy (-D warnings) and tests
bash bin/test.sh --e2e    # release build + a real --probe run
bash scripts/perf-check.sh  # CPU, RSS and wakeup budget on Linux (needs tmux)
```

UI tests render to ratatui's `TestBackend` and are checked with `insta` snapshots.
