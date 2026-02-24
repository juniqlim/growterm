# juniqterm Progress

## Phase 0: Project Setup — DONE
- Cargo workspace + juniqterm-types
- 계약 타입: Rgb, Color, CellFlags, Cell, RenderCommand, TerminalCommand, KeyEvent
- 17 unit tests

## Phase 1: GPU Draw — DONE
- wgpu + winit + fontdue
- 2-pass rendering (backgrounds + glyphs)
- Menlo + Apple SD Gothic Neo fallback (한글)
- Wide char (CJK) support
- 6 unit tests + visual verification

## Phase 2: Render Command Generator — DONE
- Pure function: Vec<Vec<Cell>> → Vec<RenderCommand>
- Color resolution (Default/Indexed 256/Rgb)
- INVERSE, DIM, HIDDEN flag handling
- 12 unit tests

## Integration (Phase 1 + 2) — DONE
- Cell → generate() → GpuDrawer::draw() pipeline verified visually

## Phase 3: VT Parser — NEXT
## Phase 4: Terminal Grid — TODO
## Phase 5: PTY I/O — TODO
## Phase 6: Input Handler — TODO
## Phase 7: App (connect all) — TODO
