# obstui

A lightweight resource monitor for the terminal, for Linux and macOS.

`obstui` shows CPU, memory, GPU, temperatures, network, disk and processes in real time,
with circular charts, history graphs with scaled axes, tmux-style navigation and
k9s-style `:` commands. It is built to stay out of the way: it sleeps until there is a new
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

Requires Rust 1.88 or newer.

```sh
cargo install --path . --locked
```

or `bash bin/deploy/prod.sh`, which does the same and prints the Intel GPU hint below.

### Intel GPU usage (Linux)

Reading Intel GPU usage needs the `CAP_PERFMON` capability on the binary:

```sh
sudo setcap cap_perfmon=ep "$(command -v obstui)"
```

The capability is lost whenever the binary is rebuilt or reinstalled; the TUI shows the
exact command when it is missing.

## Usage

```sh
obstui            # open the interface
obstui --probe    # print a one-shot diagnostic (CPU, memory, GPU, temperatures...) and exit
obstui --help
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
`~/.config/obstui/config.toml` (Linux) or
`~/Library/Application Support/obstui/config.toml` (macOS). Every key is optional:
refresh interval, history length, network interface, key bindings, an optional tmux-style
prefix key, truecolor and temperature thresholds.

## Development

```sh
bash bin/test.sh          # fmt, clippy (-D warnings) and tests
bash bin/test.sh --e2e    # release build + a real --probe run
bash scripts/perf-check.sh  # CPU, RSS and wakeup budget on Linux (needs tmux)
```

UI tests render to ratatui's `TestBackend` and are checked with `insta` snapshots.
