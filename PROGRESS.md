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

## Phase 3: VT Parser — DONE
- vte 0.13 wrapping → Vec<TerminalCommand>
- Stateful: partial/split sequences across chunks handled
- C0 controls: newline, CR, backspace, tab, bell
- CSI: cursor movement (A/B/C/D/H), erase (J/K)
- SGR: attributes (bold/dim/italic/underline/inverse/hidden/strikethrough)
- SGR: colors (basic 8, bright 8, 256-color, RGB, default)
- SGR: multiple params in one sequence
- Unicode support
- 42 unit tests

## Integration (Phase 1 + 2 + 3) — DONE
- bytes → VtParser → TerminalCommand → MiniGrid → generate() → GpuDrawer::draw()
- 이스케이프 시퀀스(색상, 속성, 커서위치, 한글)로 화면 구성 → 시각 확인 완료
- 한글 와이드 문자 간격 이슈 있음 → Phase 4 Grid에서 스페이서 셀 구조로 해결 예정

## Phase 4: Terminal Grid — DONE
- 상태머신 Grid: TerminalCommand → Vec<Vec<Cell>>
- 고정 너비 그리드: 각 행 정확히 cols개 Cell (cell index == column index)
- 와이드 문자(CJK): WIDE_CHAR 플래그 + 스페이서 셀 구조
- Print: ASCII + 와이드 문자, 줄끝 래핑, 덮어쓰기시 스페이서 정리
- 속성 상태: fg/bg/flags 유지, ResetAttributes 초기화
- 커서 이동: Up/Down/Forward/Back/Position, 1-indexed → 0-indexed, 경계 클램핑
- 줄 제어: Newline + scroll, CR, Backspace, Tab(8칸), Bell(no-op)
- Erase: EraseInLine(0/1/2), EraseInDisplay(0/1/2)
- Resize: 확장(default 채움), 축소(truncate), 커서 클램핑
- generate() 업데이트: 스페이서 셀 스킵
- 42 unit tests

## Integration (Phase 1 + 2 + 3 + 4) — DONE
- bytes → VtParser → TerminalCommand → Grid → generate() → GpuDrawer::draw()
- MiniGrid → Grid 교체 완료
- 한글 와이드 문자 스페이서 셀로 정확한 간격 처리

## Phase 5: PTY I/O — DONE
- portable-pty로 셸 프로세스 생성
- split API: spawn() → (PtyReader, PtyWriter)
  - PtyReader: impl Read, IO 스레드용
  - PtyWriter: impl Write + resize(), 메인 스레드용
- $SHELL 환경변수 기반 셸 선택 (fallback: /bin/sh)
- 4 integration tests (echo, resize, unicode, exit)

## Phase 6: Input Handler — NEXT
## Phase 7: App (connect all) — TODO
