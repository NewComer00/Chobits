# Chobits

A cross-platform live2D terminal companion living inside Zellij driven by LLM.

<img width="1917" height="967" alt="image" src="https://github.com/user-attachments/assets/254a2289-e404-48a2-b47b-63852bd28a78" />

## Build From Source

### Supported Platforms

- Linux
- Windows*

> \* You need to build this project inside 'MSYS2 UCRT64' or 'MSYS2 MINGW64' environment. After building, you can deploy and run the binaries on native Windows without MSYS2 dependencies.

### Prerequisites

| Tool           | Description                                                                           |
|----------------|---------------------------------------------------------------------------------------|
| git            | Version control system to clone the repository.                                       |
| git-lfs        | Git extension for handling large files. Install it with: \`git lfs install\`.         |
| cargo          | Rust toolchain with native and 'wasm32-wasip1' targets.                               |
| cargo-binstall | For easier installation of Zellij. Install it with: \`cargo install cargo-binstall\`. |
| jq             | JSON processor.                                                                       |
| wget           | HTTP downloader.                                                                      |
| unzip          | ZIP extractor.                                                                        |
| make           | GNU Make (for live-ascii).                                                            |
| gcc            | GNU C toolchain (for live-ascii).                                                     |

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
./build.sh --locked
```

### Manual Build

<details>
<summary>Click to expand manual build instructions</summary>

Create the local directory `install/Chobits/` to hold all binaries, configurations, live2D models and expressions:

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
for c in "" "-send" "-bar"; do cargo install --path "crates/chobits$c" --root install/Chobits/local; done
cargo install --path crates/chobits-zellij --root install/Chobits/local --target wasm32-wasip1
```

All three binaries (`chobits`, `chobits-send`, `chobits-bar`) and the WASM plugin (`chobits-zellij.wasm`) should now be in `install/Chobits/local/bin/`.

Then Install dependencies (e.g. `live-ascii` and `zellij`) according to their instructions. For convenience, just install them into the same `install/Chobits/local/bin/` directory to keep everything self-contained.

Install `live-ascii` from source, GNU Make (`make`) and C toolchain (`gcc`) required:

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

Place the pre-recorded expressions (OSF binary dumps) in `install/Chobits/expressions/`:

```bash
cp -r expressions install/Chobits/
```

#### Live2D Models

Download the Live2D model of your choice and place the `.model3.json` file somewhere accessible.  Note the path for the next step.

For example, you can download the free "Hiyori" model from Cubism, and place the extracted `hiyori_free/` directory in `install/Chobits/models/`:

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

## Deployment

Here shows the final directory structure of the newly created `Chobits/` folder under `install/` directory. We call this `Chobits/` folder **Chobits root**.

Move the `Chobits/` folder wherever you want. For MSYS2 UCRT64/MINGW64 users, you may keep it inside MSYS2 or move it to native Windows.

```
Chobits/
├── .chobits-root
├── bin
│   └── chobits-start.exe
├── config.toml
├── expressions
│   ├── blink.osf.bin
│   ├── happy.osf.bin
│   ├── lookaroud.osf.bin
│   ├── neutral.osf.bin
│   ├── sad.osf.bin
│   ├── stretch.osf.bin
│   ├── surprised.osf.bin
│   └── thinking.osf.bin
├── local
│   └── bin
│       ├── chobits.exe
│       ├── chobits-bar.exe
│       ├── chobits-send.exe
│       ├── chobits-zellij.wasm
│       ├── live-ascii.exe
│       └── zellij.exe
└── models
    └── hiyori_free
        ├── hiyori_free_t03.can3
        ├── hiyori_free_t08.cmo3
        ├── ReadMe.txt
        └── runtime
            ├── hiyori_free_t08.2048
            │   └── texture_00.png
            ├── hiyori_free_t08.cdi3.json
            ├── hiyori_free_t08.moc3
            ├── hiyori_free_t08.model3.json
            ├── hiyori_free_t08.physics3.json
            └── motion
                ├── hiyori_m01.motion3.json
                ├── hiyori_m02.motion3.json
                ├── hiyori_m03.motion3.json
                ├── hiyori_m04.motion3.json
                ├── hiyori_m05.motion3.json
                ├── hiyori_m06.motion3.json
                ├── hiyori_m07.motion3.json
                └── hiyori_m08.motion3.json
```

## Configuration

All configuration lives in `config.toml` at the Chobits root. Paths may be absolute or relative to the config file's own directory.

### `[llm]` — Language Model

The LLM backend that powers Chi's reactions — plug in any Ollama or OpenAI-compatible API.

|     Key      |          Default           |                   Description                   |
| ------------ | -------------------------- | ----------------------------------------------- |
| `backend`    | `"ollama"`                 | `"ollama"` or anything else = OpenAI-compatible |
| `url`        | `"http://localhost:11434"` | API base URL                                    |
| `model`      | `"qwen3:0.6b"`             | Model name                                      |
| `max_tokens` | `256`                      | Max tokens per response                         |
| `api_key`    | (empty)                    | API key for OpenAI-compatible backends          |

Example for ollama:

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

Controls how the Zellij plugin captures the currently focused pane (in text) and how often it polls. The snapshot taken will be sent to LLM backend when it's not busy.

If no change happens to the focused Zellij pane, no message will be sent to save tokens.

|       Key       | Default |              Description              |
| --------------- | ------- | ------------------------------------- |
| `max_bytes`     | `4096`  | Truncate snapshots (head + tail)      |
| `interval_secs` | `10`    | Plugin `dump-screen` polling interval |

```toml
[snapshot]
max_bytes     = 4096
interval_secs = 10
```

<details>
<summary>Click to expand more config items</summary>

### `[ports]` — TCP/UDP Ports

Ports for inter-process communication between the daemon, bar, and renderer.

|    Key     | Default |               Description                |
| ---------- | ------- | ---------------------------------------- |
| `snapshot` | `7878`  | TCP — daemon receives terminal snapshots |
| `bar`      | `7879`  | TCP — daemon sends text reactions        |
| `osf`      | `11573` | UDP — daemon sends OSF expression frames |

### `[live-ascii]` — Live2D ASCII Renderer

Controls the live-ascii pane — model path, input sources, protocol, and view tweaks.

|       Key        |      Default      |               Description               |
| ---------------- | ----------------- | --------------------------------------- |
| `model_set`      | (empty)           | Path to `.model3.json` file             |
| `enable_osf`     | `true`            | `--camera` (accept OSF frames)          |
| `enable_mouse`   | `true`            | `--mouse` (drag to pan, scroll to zoom) |
| `enable_physics` | `true`            | `--physics` (hair/wind physics)         |
| `image_protocol` | `"halfblock"`     | `halfblock`, `kitty`, or `sixel`        |
| `bg_color`       | `"rgba(0,0,0,0)"` | Background behind the character         |
| `scale`          | `"100%"`          | View scale percentage                   |
| `offset_x`       | `"0%"`            | Horizontal offset % of panel width      |
| `offset_y`       | `"0%"`            | Vertical offset % of panel height       |

```toml
[live-ascii]
model_set      = "models/hiyori_free/runtime/hiyori_free_t08.model3.json"
enable_osf     = true
enable_mouse   = true
enable_physics = true
image_protocol = "halfblock"
bg_color       = "rgba(0,0,0,0)"
scale          = "100%"
offset_x       = "0%"
offset_y       = "0%"
```

### `[zellij]` — Layout

Defines how Zellij arranges panes — terminal, live-ascii, bar, tab-bar, and status-bar.

The KDL layout uses templates `{live_ascii_bin}`, `{chobits_bar_bin}`, `{plugin_path}`, `{live_ascii_args}`, and `{interval_secs}` — these are filled in at launch time, so keep them as literal placeholders.

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
        pane focus=true
        pane split_direction="horizontal" size="30%" {
            pane command="{live_ascii_bin}" name="LIVE-ASCII" {
                args {live_ascii_args}
            }
            pane command="{chobits_bar_bin}" size="30%" borderless=true
        }
        pane size=1 borderless=true {
            plugin location="file:{plugin_path}" {
                chobits_send_bin "{chobits_send_bin}"
                interval_secs "{interval_secs}"
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
│    dump-screen)     │(ratatui) │
└─────────────────────┴──────────┘
```

### `[expressions]` — OSF Expression Files

Maps expression names to OSF binary dumps. The daemon scans `.osf.bin` files here and feeds the list to the LLM so it can pick one in each response.

If the current Zellij pane has not changed for `idle_timeout_secs` seconds, the character will become idle.

Users can add more expressions to the directory by [recording](#tools) them with [OpenSeeFace](https://github.com/emilianavt/OpenSeeFace) running.

```toml
[expressions]
dir               = "expressions"
idle_timeout_secs = 30
```

</details>

## Run

One-command launch:

```bash
install/Chobits/bin/chobits-start

# If you added <Chobits root>/bin/ to PATH, just run
chobits-start
```

This spawns the daemon, waits for it to be ready, then opens Zellij with the generated layout.

> [!NOTE]
> Once `chobits-start` exits, the corresponding Zellij session will be killed.

## Architecture

```
chobits-zellij  ──run_command──▶  chobits-send  ──TCP:7878──▶  chobits  ──TCP:7879──▶  chobits-bar
                                                                    │
                                                                ollama REST
                                                                    │
                                                                UDP:11573 ──▶ live-ascii
```

```
┌─ zellij ────────────────────────────────────────────────────────────┐
│  ┌─────────────────────────────┐  ┌─────────────┐  ┌────────────┐  │
│  │  main terminal              │  │  live-ascii │  │ chobits-   │  │
│  │                             │  │             │  │ bar        │  │
│  │  [chobits-zellij plugin]    │  │             │  │            │  │
│  └────────────┬────────────────┘  └──────▲──────┘  └─────▲──────┘  │
└───────────────│────────────────────────── │ ──────────────│─────────┘
                │ run_command               │ OSF UDP       │ text
                ▼                           │ :11573        │ TCP:7879
          chobits-send                      │               │
                │ TCP:7878                  │               │
                ▼                           │               │
        ┌───────────────────────────────────────────────┐   │
        │  chobits (daemon)                             │───┘
        │                                               │
        │  snapshot → LLM → { text, expression }        │
        └───────────────────┬───────────────────────────┘
                            │
                      ┌─────▼──────┐
                      │ LLM backend│
                      │ (ollama)   │
                      └────────────┘
```

### Communication Contracts

| Link                          | Protocol                            | Direction |
|-------------------------------|-------------------------------------|-----------|
| chobits-zellij → chobits-send | `run_command` (subprocess)          | one-way   |
| chobits-send → chobits        | TCP `:7878`, plain text             | one-way   |
| chobits → LLM                 | HTTP REST (ollama)                  | req/reply |
| chobits → chobits-bar         | TCP `:7879`, newline-delimited text | one-way   |
| chobits → live-ascii          | UDP `:11573`, OSF frames            | one-way   |

## Tools

| Tool                                | Description                              |
|-------------------------------------|------------------------------------------|
| `tool/openseeface_record_packet.py` | Capture raw OSF UDP frames to `.osf.bin` |
| `tool/openseeface_play_packet.py`   | Playback `.osf.bin` over UDP for testing |

Record from a live OpenSeeFace session:

```bash
python tool/openseeface_record_packet.py neutral.osf.bin
```

Test playback independently:

```bash
python tool/openseeface_play_packet.py neutral.osf.bin --loop
```

## Related Projects

- [NewComer00/live-ascii](https://github.com/NewComer00/live-ascii) (forked from [Arcelyth/live-ascii](Arcelyth/live-ascii), Copyright (c) 2026 Arcelyth, MIT License)

## License

MIT
