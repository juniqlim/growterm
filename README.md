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
| Shared Types | Core data types (`Cell`, `Color`, `RenderCommand`, `TerminalCommand`, `KeyEvent`, etc.). Common vocabulary shared across all modules. |
| Input Encoding | Converts `KeyEvent` (key + modifiers) into PTY-compatible byte sequences using xterm escape codes. Bridges user keyboard input to what the shell process expects. |
| PTY | Spawns and manages a shell process in a pseudo-terminal (`PtyReader`, `PtyWriter`, `PtyResponder`). Bidirectional I/O bridge — sends encoded input to shell, receives raw output bytes. |
| VT Parser | Parses raw terminal output bytes via the `vte` crate, emitting `TerminalCommand`s (print, cursor moves, SGR attributes). Converts opaque byte stream into structured, actionable commands. |
| Grid | 2D cell buffer maintaining cursor position, scrollback history, and current styling state. Applies `TerminalCommand`s sequentially to mutate grid state; exposes `visible_cells()` for rendering. |
| Render Commands | Converts grid cells + cursor/selection/preedit overlays into `RenderCommand`s, resolving colors to RGB. Final CPU-side preparation — every cell becomes a draw instruction with position, character, colors, and flags. |
| GPU Rendering | `GlyphAtlas` rasterizes characters to bitmaps; `GpuDrawer` consumes `RenderCommand`s and renders via Metal. Executes the actual visual output on screen using GPU acceleration. |
| macOS | Native macOS integration: window lifecycle, IME input, event handling, and app delegate. Platform layer that provides native events and bridges them into the app layer. |
| App | Main event loop coordinator: reads PTY output, dispatches events, orchestrates the full grid → render pipeline. Synchronizes all components and manages timing (frame rate, resize, input delivery). |

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
