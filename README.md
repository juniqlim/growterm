<p align="center">
  <img src="assets/banner.svg" alt="growTerm banner" width="100%"/>
</p>

[한국어](README.ko.md)

A terminal app that grows — GPU-accelerated terminal emulator written in Rust for macOS.

## Design Goals

- **Modular**: Each module has a single responsibility. You don't need to know the VT parser to fix clipboard copy.
- **Testable**: Pure functions and state machines are verified with unit tests; module interactions with integration tests.
- **Evolvable**: Reversible structure makes it safe to change, grow, and evolve.

## Features

- **GPU Rendering** — wgpu-based 2-pass rendering (background + glyphs)
- **Korean Support** — IME input with preedit overlay, wide character handling, D2Coding font
- **VT Parsing** — SGR attributes (bold, dim, italic, underline, strikethrough, inverse), 256/RGB color, cursor movement, screen clearing
- **Scrollback** — 10,000 line history, Cmd+PageUp/PageDown, visual scrollbar
- **Mouse Selection & Clipboard** — Drag selection with wide character awareness, Cmd+C/V
- **Font Zoom** — Cmd+=/- to adjust size (8pt–72pt)
- **Box Drawing** — Light, heavy, double, and rounded corner characters with geometric rendering
- **Keyboard** — xterm-style encoding, Shift/Ctrl/Alt modifier combinations

## Architecture

```
Key Input → Input Encoding → PTY
                              ↓
                           VT Parser
                              ↓
                             Grid
                              ↓
                        Render Commands
                              ↓
                         GPU Rendering → Screen
```

| Module | Role |
|---|---|
| Shared Types | Cell, Color, RenderCommand, etc. |
| VT Parser | VT100/xterm escape sequence parsing |
| Grid | Terminal grid state management |
| Render Commands | Grid → Render command conversion |
| GPU Rendering | wgpu-based screen output |
| Input Encoding | Key input → PTY byte conversion |
| PTY | Shell process management |
| App | Event loop, module integration |

## Build & Run

```bash
cargo build --release
cargo run -p growterm-app
```

### Install as macOS App

```bash
./install.sh
```

Builds the release binary and installs `growTerm.app` to `/Applications`.

## Test

```bash
cargo test
```

258+ tests (unit + integration).

## Requirements

- Rust (stable)
- macOS (wgpu Metal backend)
