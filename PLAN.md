# juniqterm 구현 계획

## Context

기존 터미널 오픈소스 프로젝트는 기여 진입장벽이 높다. 모듈 간 결합이 강해서 한 부분을 고치려면 전체를 알아야 하고, 테스트가 부족해서 변경이 안전한지 확인할 수 없다.

juniqterm은 **"내 변경이 안전한지 테스트로 직접 확인할 수 있는 터미널"**을 목표로 한다. 모듈 간 계약이 테스트로 명세되어 있어서, 기여자가 자기 모듈 테스트만 돌려도 나머지에 영향 없다는 확신을 가질 수 있다.

- 언어: **Rust** (사용자가 wgpu 경험 있음, `~/j/GPUSortedMap`)
- 플랫폼: macOS 우선 (모듈화로 나중에 교체 가능)
- 위치: `~/j/juniqterm`

## 프로젝트 구조

```
~/j/juniqterm/
  Cargo.toml              # workspace
  README.md
  crates/
    juniqterm-types/       # 모듈 간 계약 (공유 타입)
    juniqterm-gpu-draw/    # GPU 드로우 (바보 렌더러)
    juniqterm-render-cmd/  # Grid → RenderCommand 변환
    juniqterm-vt-parser/   # bytes → TerminalCommand
    juniqterm-grid/        # TerminalCommand → Grid 상태
    juniqterm-pty/         # 셸 프로세스 통신
    juniqterm-input/       # KeyEvent → bytes
    juniqterm-app/         # 모든 모듈을 연결하는 바이너리
```

**의존성 규칙: 모듈은 서로 의존하지 않는다. 오직 `juniqterm-types`만 공유한다.**

```
juniqterm-types  ← 모든 모듈이 의존
    ↑
    ├── gpu-draw      (+ wgpu, winit, fontdue)
    ├── render-cmd    (외부 의존성 없음)
    ├── vt-parser     (+ vte)
    ├── grid          (외부 의존성 없음)
    ├── pty           (+ portable-pty)
    ├── input         (외부 의존성 없음)
    │
    └── app           (모든 모듈 + winit)
```

## 데이터 흐름

```
키보드 → [Input] → PTY → [VT Parser] → [Grid] → [Render Cmd Gen] → [GPU Draw] → 화면
```

## 구현 순서

### Phase 0: 프로젝트 셋업
- Cargo workspace 생성
- `juniqterm-types` 크레이트: 모든 계약 타입 정의
  - `RenderCommand` (GPU Draw 입력)
  - `Cell`, `Grid` 인터페이스 (그리드 상태)
  - `TerminalCommand` (VT 파서 출력)
  - `KeyEvent` (입력 핸들러 입력)
  - `Rgb`, `Color`, `CellFlags`
- types 테스트: Default 값, 비트플래그 조합 등
- git init, 초기 커밋

### Phase 1: GPU Draw (시각 확인 후 잠금)
- wgpu + winit으로 창 생성
- `fontdue`로 글리프 래스터화 + 텍스처 아틀라스
- 2-pass 렌더링: (1) 셀 배경색, (2) 텍스트
- 공개 API: `GpuDrawer::draw(commands: &[RenderCommand])`
- 하드코딩된 그리드를 화면에 그리는 example로 시각 확인
- **확인 후 이 모듈은 거의 안 건드림**
- 테스트: 아틀라스 캐싱, 래스터화 결과 유무 (시각 확인은 수동)

### Phase 2: Render Command Generator (테스트 가능)
- `Grid → Vec<RenderCommand>` 순수 함수
- Color 해석 (Named/Indexed → Rgb), INVERSE/DIM/HIDDEN 처리
- 와이드 문자(CJK) 스페이서 스킵
- 테스트: 빈 그리드, 색상 반전, 와이드 문자 등

### Phase 3: VT Parser (테스트 가능)
- `vte` 크레이트 래핑 → `Vec<TerminalCommand>` 출력
- 상태 유지 (청크 간 끊긴 시퀀스 처리)
- 테스트: ASCII, CSI 시퀀스, SGR, 부분 시퀀스 등

### Phase 4: Terminal Grid (테스트 가능, 가장 두꺼운 테스트)
- `TerminalCommand` 적용 → Grid 상태 업데이트
- 커서, 줄바꿈, 스크롤, 스크롤백 버퍼
- 테스트: 출력, 줄바꿈 스크롤, 커서 이동, SGR 적용, 지우기 등

### Phase 5: PTY I/O (통합 테스트)
- `portable-pty`로 셸 프로세스 생성
- trait `PtyHandle`: read, write, resize
- 통합 테스트: echo 명령 → 출력 확인

### Phase 6: Input Handler (테스트 가능)
- `KeyEvent → Vec<u8>` 순수 함수
- 화살표 키, Ctrl 조합, 유니코드 등
- 테스트: 각 키 → 예상 바이트

### Phase 7: 연결 + MVP
- winit 이벤트 루프에서 모든 모듈 연결
- IO 스레드: PTY → VT Parser → Grid
- 메인 스레드: 키 입력 → PTY, Grid → Render Cmd → GPU Draw
- 60fps, dirty flag로 변경 시에만 렌더링

## 핵심 계약 타입 (요약)

```rust
// GPU Draw가 받는 것
struct RenderCommand { col: u16, row: u16, character: char, fg: Rgb, bg: Rgb, flags: CellFlags }

// Grid의 셀
struct Cell { character: char, fg: Color, bg: Color, flags: CellFlags }

// VT Parser가 내보내는 것
enum TerminalCommand { Print(char), CursorUp(u16), SetGraphicsRendition(...), ... }

// Input Handler가 받는 것
struct KeyEvent { key: Key, modifiers: Modifiers }
```

## 주요 외부 크레이트

| 크레이트 | 용도 | 모듈 |
|---------|------|------|
| wgpu | GPU 렌더링 | gpu-draw |
| winit | 윈도우/이벤트 | gpu-draw, app |
| fontdue | 글리프 래스터화 (순수 Rust) | gpu-draw |
| bytemuck | GPU 버퍼 변환 | gpu-draw |
| vte | VT 파서 상태 머신 | vt-parser |
| portable-pty | PTY 통신 | pty |
| bitflags | CellFlags | types |

## 검증 방법

- Phase 0~6: 각 크레이트에서 `cargo test` → 모듈 단독 검증
- Phase 1: `cargo run --example hardcoded_grid` → 시각 확인
- Phase 5: 통합 테스트 (`tests/` 디렉토리, 단위 테스트와 분리)
- Phase 7: `cargo run -p juniqterm-app` → 실제 셸 동작 확인
- 전체: `cargo test --workspace` → 모든 모듈 한번에 검증
