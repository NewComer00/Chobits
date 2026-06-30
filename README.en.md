# Chobits

<p align="left">
  <a href="README.md"><img src="https://img.shields.io/badge/语言-简体中文-red.svg"></a>
  <a href="README.en.md"><img src="https://img.shields.io/badge/lang-English-blue.svg"></a>
</p>

A cross-platform Live2D terminal companion living inside Zellij driven by LLM.

<img width="1917" height="967" alt="image" src="https://github.com/user-attachments/assets/254a2289-e404-48a2-b47b-63852bd28a78" />

## Supported Platforms

- Linux
- macOS†
- Windows*

> † **macOS:** [pre-built binaries](https://github.com/NewComer00/Chobits/releases) are not notarized and may be blocked by Gatekeeper. Either [build from source](#build-from-source) or follow [Apple's instructions](https://support.apple.com/en-us/102445) to allow the app manually.

> \* **Building** on Windows requires MSYS2 UCRT64 or MINGW64. **Pre-built release archives** run on native Windows without MSYS2.

## Quick Start

> [!NOTE]  
> The release package includes a free model ["Hiyori"](https://www.live2d.com/en/learn/sample/momose-hiyori/) downloaded from [Cubism](https://www.live2d.com/en/learn/sample/).
> 
> Before using this model, please review the ["Free Material License Agreement"](https://www.live2d.com/eula/live2d-free-material-license-agreement_en.html) and the ["Live2D Cubism Sample Data Terms of Use"](https://www.live2d.com/learn/sample/model-terms/).

Install the [latest release](https://github.com/NewComer00/Chobits/releases/latest) with one command. The installer adds `chobits-start` to your user PATH and prints `[llm]` examples for `config.toml`.

### Install on Linux

Requires Bash or Zsh. Default install location: `~/.local/share/Chobits`.

```bash
. <(curl -LsSf https://raw.githubusercontent.com/NewComer00/Chobits/main/scripts/install.sh)
```

### Install on macOS

Requires Bash or Zsh. Default install location: `~/.local/share/Chobits`.

Open a new terminal after installation for PATH changes to take effect.

```bash
curl -LsSf https://raw.githubusercontent.com/NewComer00/Chobits/main/scripts/install.sh | sh
```

### Install on Windows

Requires PowerShell 5.1+. Default install location: `%LOCALAPPDATA%\Chobits`.

```powershell
irm https://raw.githubusercontent.com/NewComer00/Chobits/main/scripts/install.ps1 | iex
```

### Configure LLM Backend

Edit `config.toml` before your first run — by default at `~/.local/share/Chobits/config.toml` (Linux / macOS) or `%LOCALAPPDATA%\Chobits\config.toml` (Windows).

Find the `[llm]` section in the config. Chobits supports the following LLM backends

**Ollama**

```toml
[llm]
backend    = "ollama"
url        = "http://localhost:11434"
model      = "qwen3:0.6b"
max_tokens = 512
```

**OpenAI-compatible** (specify anything other than `ollama` at `backend` field)

```toml
[llm]
backend    = "deepseek"
url        = "https://api.deepseek.com"
model      = "deepseek-v4-flash"
max_tokens = 512
api_key    = "sk-..."
```

🎉 You're all set! Just run `chobits-start` to launch Chobits. 🚀

> [!TIP]  
> `chobits-start` creates or re-attaches to a dedicated Zellij session — the daemon, live-ascii, bar, and plugin all run inside it, separate from your normal terminal sessions.
>
> - **First launch** — a new session is created automatically.
> - **Later runs** — `chobits-start` re-attaches to the existing session (you pick one if several are open).
> - **Detach** without stopping — press `Ctrl+o` then `d` in Zellij; run `chobits-start` again to come back.
> - **Quit entirely** — press `Ctrl+q` or close all panes.
>
> See [Run](#run) for subcommands and session management.

For manual installs, see [Download from Release](#download-from-release) or [Build from Source](#build-from-source). For all settings, see [Configuration](#configuration).

<details>
<summary>Click to expand more installer options</summary>

### Installer options

|         Variable         |                       Default                       |                  Description                  |
| ------------------------ | --------------------------------------------------- | --------------------------------------------- |
| `CHOBITS_INSTALL_DIR`    | `~/.local/share/Chobits` / `%LOCALAPPDATA%\Chobits` | Install location                              |
| `CHOBITS_VERSION`        | `latest`                                            | Release tag (e.g. `v0.2.0`)                   |
| `CHOBITS_LIBC`           | `musl`                                              | Linux only: `musl` or `gnu`                   |
| `CHOBITS_NO_MODIFY_PATH` | (unset)                                             | Set to `1` to skip adding `bin/` to user PATH |

</details>

## Download from Release

<details>
<summary>Click to expand</summary>

Pre-built binaries are available on the [Releases](https://github.com/NewComer00/Chobits/releases) page for the following platforms:

|                  Package                   |      Platform       |                   Notes                   |
| ------------------------------------------ | ------------------- | ----------------------------------------- |
| `Chobits-x86_64-unknown-linux-gnu.tar.gz`  | x86_64 Linux        | Standard glibc-linked build               |
| `Chobits-x86_64-unknown-linux-musl.tar.gz` | x86_64 Linux        | Lightweight, static-linked musl build     |
| `Chobits-aarch64-apple-darwin.tar.gz`      | Apple Silicon macOS | arm64 build for M-series Macs             |
| `Chobits-x86_64-pc-windows-gnu.zip`        | x86_64 Windows      | Runs on native Windows; no MSYS2 required |

Download and extract the archive for your platform:

### Linux

To install Chobits on Linux, first download the latest static MUSL build and extract it:

```bash
wget https://github.com/NewComer00/Chobits/releases/latest/download/Chobits-x86_64-unknown-linux-musl.tar.gz
tar -xzf Chobits-x86_64-unknown-linux-musl.tar.gz
```

The static MUSL build (`Chobits-x86_64-unknown-linux-musl.tar.gz`) is recommended for broad compatibility across most Linux distributions. If you are on a glibc-based system, you may alternatively use `Chobits-x86_64-unknown-linux-gnu.tar.gz`.

### macOS

Download the archive for M-series Macs:

```bash
curl -LO https://github.com/NewComer00/Chobits/releases/latest/download/Chobits-aarch64-apple-darwin.tar.gz
tar -xzf Chobits-aarch64-apple-darwin.tar.gz
```

> [!NOTE]  
> The binaries are not notarized. If macOS blocks `chobits-start` on first run, follow [Apple's instructions](https://support.apple.com/en-us/102445) to allow it manually.

### Windows

Download the latest Windows release:

```powershell
Invoke-WebRequest -Uri "https://github.com/NewComer00/Chobits/releases/latest/download/Chobits-x86_64-pc-windows-gnu.zip" -OutFile "Chobits-x86_64-pc-windows-gnu.zip"
Expand-Archive -Path "Chobits-x86_64-pc-windows-gnu.zip" -DestinationPath .
```

This will extract a `Chobits/` directory containing all necessary files. You can move the `Chobits` folder anywhere you prefer.

To proceed, continue with the [Deployment](#deployment) instructions.

</details>

## Build from Source

<details>
<summary>Click to expand</summary>

### Prerequisites

|      Tool      |                                      Description                                      |
| -------------- | ------------------------------------------------------------------------------------- |
| git            | Version control system to clone the repository.                                       |
| git-lfs        | Git extension for handling large files. Install it with: \`git lfs install\`.         |
| cargo          | Rust toolchain with native and 'wasm32-wasip1' targets.                               |
| cargo-binstall | For easier installation of Zellij. Install it with: \`cargo install cargo-binstall\`. |
| jq             | JSON processor.                                                                       |
| wget           | HTTP downloader.                                                                      |
| unzip          | ZIP extractor.                                                                        |
| make           | GNU Make (for live-ascii).                                                            |
| cc             | C toolchain (for live-ascii).                                                         |

For MSYS2 UCRT64/MINGW64 users, you can install these tools with:

``` bash
pacman -S ${MINGW_PACKAGE_PREFIX}-{git,git-lfs,rust,rust-wasm,jq,wget,gcc} unzip make
cargo install cargo-binstall  # This may take a while to compile
```

### Automated Build

```bash
git lfs install
git clone --depth 1 https://github.com/NewComer00/Chobits.git
cd Chobits
# git checkout v0.2.0   # optional: match a release tag
./scripts/build.sh --locked -y
```

### Manual Build

<details>
<summary>Click to expand manual build instructions</summary>

Create the local directory `install/Chobits/` to hold all binaries, configurations, and Live2D models:

```bash
mkdir -p install/Chobits
```

#### Entrypoint Binaries

Install the entrypoint executable `chobits-start` to the directory `install/Chobits/bin/`:

```bash
cargo install --path "crates/chobits-start" --root install/Chobits
```

#### Local Binaries

Install other binaries to the directory `install/Chobits/local/bin/`:

```bash
for c in "" "-bar"; do cargo install --path "crates/chobits$c" --root install/Chobits/local; done
cargo install --path crates/chobits-zellij --root install/Chobits/local --target wasm32-wasip1
```

The `chobits` and `chobits-bar` binaries and the WASM plugin (`chobits-zellij.wasm`) should now be in `install/Chobits/local/bin/`.

Then install dependencies (e.g. `live-ascii` and `zellij`) according to their instructions. For convenience, just install them into the same `install/Chobits/local/bin/` directory to keep everything self-contained.

Install `live-ascii` from source, GNU Make (`make`) and C toolchain (`cc`) required:

```bash
cargo install --git https://github.com/NewComer00/live-ascii --root install/Chobits/local
```

Install `zellij` from source or get the latest release binary with `cargo-binstall` tool:

```bash
# Get the version of zellij from Cargo.toml to ensure compatibility with the plugin
ZELLIJ_VER=$(cargo metadata --format-version 1 | jq -r '.packages[] | select(.name == "zellij-tile") | .version')

# Install zellij from source:
cargo install zellij --version ${ZELLIJ_VER} --root install/Chobits/local

# or get the latest release binary with `cargo-binstall` tool:
# cargo binstall zellij@${ZELLIJ_VER} --root install/Chobits/local

# For MSYS2 UCRT64/MINGW64 users, the simplest way is to download the pre-built binary from GitHub releases:
# wget https://github.com/zellij-org/zellij/releases/download/v${ZELLIJ_VER}/zellij-x86_64-pc-windows-msvc.zip
# unzip zellij-x86_64-pc-windows-msvc.zip -d install/Chobits/local/bin
```

Now you should have `live-ascii` and `zellij` binaries in `install/Chobits/local/bin/` as well.

#### Expressions

Configure motion/expression aliases in `config.toml` (see `[vts.motion_alias]`). Inspect available VTS hotkeys with:

```bash
python tool/list_vts_hotkeys.py
```

#### Live2D Models

Download the Live2D model of your choice and place the `.model3.json` file somewhere accessible.  Note the path for the next step.

For example, you can download the free ["Hiyori"](https://www.live2d.com/en/learn/sample/momose-hiyori/) model from [Cubism](https://www.live2d.com/en/learn/sample/), and place the extracted `hiyori_free/` directory in `install/Chobits/models/`:

```bash
mkdir -p install/Chobits/models
wget https://cubism.live2d.com/sample-data/bin/hiyori/hiyori_en.zip
unzip hiyori_en.zip
cp hiyori_free install/Chobits/models/ -r
```

#### Config File

Copy the example configuration file to `install/Chobits/config.toml`:

```bash
cp example_config.toml install/Chobits/config.toml
```

</details>

### Format, Lint, and Test

```bash
cargo fmt --all --check && cargo clippy-all && cargo test-all && cargo check -p chobits-zellij --target wasm32-wasip1
```

</details>

## Deployment

<details>
<summary>Click to expand</summary>

This is the final directory structure of the `Chobits/` folder (under `install/` when built from source, or at the top level when extracted from a release). We call this folder the **Chobits root**.

Move the `Chobits/` folder wherever you want. For MSYS2 UCRT64/MINGW64 users, you may keep it inside MSYS2 or move it to native Windows.

```
Chobits/
├── .chobits-root
├── bin/
│   └── chobits-start          # .exe on Windows
├── config.toml
├── .zellij/                   # Zellij config/data ([zellij] paths)
├── .chobits/
│   └── vts_token.json           # saved VTS plugin auth token (auto-created)
├── local/
│   └── bin/
│       ├── chobits
│       ├── chobits-bar
│       ├── chobits-zellij.wasm
│       ├── live-ascii
│       └── zellij               # .exe on Windows
└── models/
    └── hiyori_free/
        └── runtime/
            └── hiyori_free_t08.model3.json  (+ textures, motions, …)
```

</details>

## Configuration

All configuration lives in `config.toml` at the Chobits root. The default configuration file generated by [Quick Start](#quick-start) is located at `~/.local/share/Chobits/config.toml` (Linux / macOS) or `%LOCALAPPDATA%\Chobits\config.toml` (Windows).

Paths may be absolute or relative to the **Chobits root** (the folder containing `config.toml`), regardless of where you launch the app from.

### `[llm]` — Language Model

The LLM backend that powers Chi's reactions — plug in any Ollama or OpenAI-compatible API.

|     Key      |          Default           |                   Description                   |
| ------------ | -------------------------- | ----------------------------------------------- |
| `backend`    | `"ollama"`                 | `"ollama"` or anything else = OpenAI-compatible |
| `url`        | `"http://localhost:11434"` | API base URL                                    |
| `model`      | `"qwen3:0.6b"`             | Model name                                      |
| `max_tokens` | `512`                      | Max tokens per response                         |
| `api_key`    | (empty)                    | API key for OpenAI-compatible backends          |

Example for Ollama:

```toml
[llm]
backend    = "ollama"
url        = "http://localhost:11434"
model      = "qwen3:0.6b"
max_tokens = 512
```

Example for other OpenAI-compatible provider (`backend != "ollama"`):

```toml
[llm]
backend    = "deepseek"
url        = "https://api.deepseek.com"
model      = "deepseek-v4-flash"
max_tokens = 512
api_key    = "sk-..."
```

### `[persona]` — Character

Define who the character is. The description shapes every reaction.

|      Key      |   Default   |               Description                |
| ------------- | ----------- | ---------------------------------------- |
| `name`        | `"Chi"`     | Character name used in the system prompt |
| `description` | (see below) | Personality description for the LLM      |

```toml
[persona]
name        = "Chi"
description = """
Curious and warm terminal companion.
You speak in short, casual reactions — one or two sentences max.
You genuinely care about what the user is working on.
"""
```

### `[snapshot]` — Terminal Polling

Controls how the Zellij plugin captures the currently focused pane (in text) and how often it polls. Snapshots are truncated to `max_bytes`, then posted to `http://127.0.0.1:{port}/snapshot` via Zellij's `web_request` API (localhost only — not exposed on your LAN). The daemon forwards changed snapshots to the LLM when it is not busy.

If the focused pane content is unchanged since the last poll, the daemon skips the LLM call to save tokens.

|       Key       | Default |               Description                |
| --------------- | ------- | ---------------------------------------- |
| `port`          | `7880`  | Localhost HTTP — plugin `POST /snapshot` |
| `max_bytes`     | `4096`  | Truncate snapshots (head + tail)         |
| `interval_secs` | `10`    | Plugin pane polling interval             |

```toml
[snapshot]
port          = 7880
max_bytes     = 4096
interval_secs = 10
```

### `[bar]` — Text Reaction Bar

Controls the chobits-bar scrollback pane. Mouse wheel scrolls history; new messages auto-scroll only when you are already at the bottom. Press `q`, `Esc`, or `Ctrl+C` to quit the bar pane.

|       Key        | Default |              Description              |
| ---------------- | ------- | ------------------------------------- |
| `port`           | `7879`  | TCP — daemon sends text reactions     |
| `history_length` | `50`    | Max text reactions kept in scrollback |

```toml
[bar]
port           = 7879
history_length = 50
```

<details>
<summary>Click to expand more config items</summary>

### `[live-ascii]` — Live2D ASCII Renderer

Controls the live-ascii pane — model path, input sources, protocol, and view tweaks.

When the live-ascii pane has focus, use the arrow keys or mouse drag to move the model. You can also use the plus/minus keys or the mouse wheel to resize the model.

|       Key        |      Default      |               Description               |
| ---------------- | ----------------- | --------------------------------------- |
| `model_set`      | (empty)           | Path to `.model3.json` file             |
| `enable_vts`     | `true`            | `--vts` (VTS API server for hotkeys)    |
| `vts_port`       | `8001`            | `--vts-port`                            |
| `enable_mouse`   | `true`            | `--mouse` (drag to pan, scroll to zoom) |
| `enable_physics` | `true`            | `--physics` (hair/wind physics)         |
| `image_protocol` | `"halfblock"`     | `halfblock`, `kitty`, or `sixel`        |
| `bg_color`       | `"rgba(0,0,0,0)"` | Background behind the character         |
| `scale`          | `"100%"`          | View scale percentage                   |
| `offset_x`       | `"0%"`            | Horizontal offset % of panel width      |
| `offset_y`       | `"0%"`            | Vertical offset % of panel height       |

Bundled example (Hiyori-tuned scale/offset):

```toml
[live-ascii]
model_set      = "models/hiyori_free/runtime/hiyori_free_t08.model3.json"
enable_vts     = true
vts_port       = 8001
enable_mouse   = true
enable_physics = true
image_protocol = "halfblock"
bg_color       = "rgba(0,0,0,0)"
scale          = "550%"
offset_x       = "0%"
offset_y       = "95%"
```

### `[zellij]` — Layout

Defines how Zellij arranges panes — terminal, live-ascii, bar, tab-bar, and status-bar.

The KDL layout uses templates `{chobits_bin}`, `{plugin_path}`, `{live_ascii_bin}`, `{chobits_bar_bin}`, `{live_ascii_args}`, `{interval_secs}`, `{max_bytes}`, `{snapshot_port}`, etc. — these are filled in at launch time, so keep them as literal placeholders.

On each launch, `chobits-start` writes the resolved layout to `.zellij/config/layouts/layout.kdl` and pre-grants the WASM plugin permissions (`ReadApplicationState`, `ReadPaneContents`, `WebAccess`).

```toml
[zellij]
config_dir = ".zellij/config"
data_dir   = ".zellij/data"
layout     = """
layout {
    pane size=1 borderless=true {
        plugin location="tab-bar"
    }
    pane split_direction="vertical" {
        pane size=1 borderless=true command="{chobits_bin}" {
            args "--quiet"
        }
        pane focus=true
        pane split_direction="horizontal" size="30%" {
            pane command="{live_ascii_bin}" name="LIVE-ASCII" {
                args {live_ascii_args}
            }
            pane command="{chobits_bar_bin}" size="30%" borderless=true
        }
        pane size=1 borderless=true {
            plugin location="file:{plugin_path}" {
                snapshot_port "{snapshot_port}"
                interval_secs "{interval_secs}"
                max_bytes "{max_bytes}"
            }
        }
    }
    pane size=1 borderless=true {
        plugin location="status-bar"
    }
}
"""
```

```
┌─────────────────────┬──────────┐
│   terminal          │live-ascii│
│   (zellij native    │          │
│    with plugin      ├──────────┤
│    polling via      │chi bar   │
│    pane scrollback) │(ratatui) │
└─────────────────────┴──────────┘
```

### `[idle]` — Idle Behavior

Controls idle monologue timing. Only labels listed in `[vts.motion_alias]` / `[vts.expression_alias]` are sent to the LLM.

|         Key         | Default |                       Description                        |
| ------------------- | ------- | -------------------------------------------------------- |
| `idle_timeout_secs` | `30`    | Seconds of pane inactivity before idle behavior          |

```toml
[idle]
idle_timeout_secs = 30
```

Legacy configs may still use `[expressions]` with the same key.

### `[vts]` — VTS Plugin Client

The daemon connects to live-ascii's built-in [VTube Studio API](https://github.com/NewComer00/live-ascii) server as a plugin client to trigger hotkeys.

|           Key            |                Default                 |                    Description                     |
| ------------------------ | -------------------------------------- | -------------------------------------------------- |
| `url`                    | `"ws://127.0.0.1:8001"`                | VTS WebSocket URL (port should match `vts_port`)   |
| `plugin_name`            | `"Chobits"`                            | Plugin name shown in VTS                           |
| `developer`              | `"Chobits"`                            | Developer name for authentication                  |
| `auth_token_path`        | `".chobits/vts_token.json"`            | Saved auth token (auto-created on first connect)   |
| `connect_timeout_secs`   | `30`                                   | Retry connecting until live-ascii VTS is ready     |

Map friendly LLM labels to VTS hotkey names (the `name` column from `list_vts_hotkeys.py`) or internal slugs (e.g. `idle_2`). Array values pick randomly at runtime.

```toml
[vts]
url                  = "ws://127.0.0.1:8001"
plugin_name          = "Chobits"
developer            = "Chobits"
auth_token_path      = ".chobits/vts_token.json"
connect_timeout_secs = 30

[vts.motion_alias]
idle      = "Idle #2"
happy     = "Idle #1"
thinking  = "Flick #0"       # wait-loop while LLM runs; also an LLM alias
worried   = "Flickdown #0"
surprised = "Tap #0"
sad       = "Flick@Body #0"

# Models with .exp3.json expressions can add [vts.expression_alias]
# happy = "My Expression Name"
```

</details>

## Run

### Launch

If you used the [Quick Start](#quick-start) installer, run `chobits-start` from anywhere.

```bash
chobits-start
```

Manual install or release archive — run from a directory that can reach your **Chobits root**:

```bash
Chobits/bin/chobits-start
```

Config lives at `<Chobits root>/config.toml`. Edit `[llm]` before your first run — see [Quick Start](#quick-start) for examples.

On first launch a new Zellij session is created. On subsequent runs,
`chobits-start` detects the existing session and re-attaches automatically.
If multiple sessions are running, you will be prompted to select one.

To detach from the session without stopping it, press `Ctrl+o` then `d` inside Zellij.
Running `chobits-start` again will re-attach. To terminate the session entirely,
press `Ctrl+q` or close all panes.

> [!NOTE]  
> After upgrading Chobits, run `chobits-start` once so Zellij picks up layout changes
> (e.g. `snapshot_port`) and refreshed plugin permissions.

### Subcommands

Pass arguments directly to the bundled Zellij instance:

```bash
chobits-start zellij <args>

# Examples
chobits-start zellij ls                  # list sessions
chobits-start zellij attach --session <name>
chobits-start zellij --help
```

This is equivalent to running `zellij --config-dir ... --data-dir ... <args>`
with the correct isolated paths — no need to know where they are.

> [!NOTE]  
> Detaching (`Ctrl+o` then `d`) pauses terminal snapshot polling, so there will be no LLM calls from
> screen changes while no client is attached.

## Architecture

`chobits-start` launches Zellij with the daemon, live-ascii, chobits-bar, and the `chobits-zellij` WASM plugin. Default localhost ports:

|  Port   |        Config key        | Protocol  |            Purpose             |
| ------- | ------------------------ | --------- | ------------------------------ |
| `7880`  | `[snapshot] port`        | HTTP      | Plugin → daemon snapshots      |
| `7879`  | `[bar] port`             | TCP       | Daemon → chobits-bar reactions |
| `8001`  | `[vts] url` / `[live-ascii] vts_port` | WebSocket | Daemon → live-ascii VTS API |

**Data flow** (while a client is attached):

```
chobits-start ──▶ zellij session (layout from config.toml)
                      │
chobits-zellij ──get_pane_scrollback──▶ snapshot JSON
                      │
                      └──HTTP POST :7880──▶ chobits ──┬── TCP:7879 ──▶ chobits-bar
                                              ├── HTTP REST ──▶ LLM backend
                                              └── WebSocket:8001 ──▶ live-ascii (--vts)
```

When no client is attached (detached), the plugin skips pane polling.

**Layout** (inside Zellij):

```mermaid
flowchart TB
    subgraph Zellij["Zellij session"]
        direction LR
        Terminal["Main terminal<br/>chobits-zellij plugin"]
        Daemon["chobits daemon"]
        LiveAscii["live-ascii"]
        Bar["chobits-bar"]
    end

    LLM["LLM backend<br/>(Ollama or compatible API)"]

    Terminal -->|"HTTP POST :7880"| Daemon
    Daemon -->|"HTTP REST"| LLM
    Daemon -->|"WebSocket :8001"| LiveAscii
    Daemon -->|"TCP :7879"| Bar
```

### Communication Contracts

|           Link           |                          Protocol                          | Direction |
| ------------------------ | ---------------------------------------------------------- | --------- |
| chobits-zellij → chobits | Zellij `web_request` → HTTP POST `127.0.0.1:7880/snapshot` | one-way   |
| chobits → LLM            | HTTP REST (Ollama or OpenAI-compatible)                    | req/reply |
| chobits → chobits-bar    | TCP `:7879` (default), newline-delimited text              | one-way   |
| chobits → live-ascii     | WebSocket `:8001` (default), VTS `HotkeyTriggerRequest`    | one-way   |

## Tools

|                Tool                 |               Description                |
| ----------------------------------- | ---------------------------------------- |
| `tool/list_vts_hotkeys.py`          | List live-ascii VTS hotkeys for configuring `[vts.*_alias]` |

List hotkeys from a running live-ascii instance (requires `pip install websockets`):

```bash
# Terminal 1: live-ascii with --vts on your model
# Terminal 2:
python tool/list_vts_hotkeys.py
```

Copy the printed hotkey names into `[vts.motion_alias]` / `[vts.expression_alias]` in `config.toml`.

## Related Projects

- [NewComer00/live-ascii](https://github.com/NewComer00/live-ascii) (forked from [Arcelyth/live-ascii](https://github.com/Arcelyth/live-ascii), Copyright (c) 2026 Arcelyth, MIT License)

## License

MIT
