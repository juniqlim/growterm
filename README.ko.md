<p align="center">
  <img src="assets/banner.svg" alt="growTerm banner" width="100%"/>
</p>

[English](README.md)

자라나는 터미널 앱 — Rust로 만든 GPU 가속 터미널 에뮬레이터. macOS 지원.

## 설계 목표

- **Modular**: 각 모듈은 하나의 책임만 갖는다. 클립보드 복사를 고칠 때 VT 파서를 몰라도 된다.
- **Testable**: 순수 함수와 상태 머신은 단위 테스트로, 모듈 간 연동은 통합 테스트로 검증한다.
- **Evolvable**: 가역적 구조라서 안심하고 변화하고, 성장하고, 진화할 수 있다.

## 특징

- **GPU 렌더링** — wgpu 기반 2-pass 렌더링 (배경 + 글리프)
- **한글 지원** — IME 입력 + 프리에딧 오버레이, 와이드 문자 처리, D2Coding 폰트
- **VT 파싱** — SGR 속성 (볼드, 딤, 이탤릭, 밑줄, 취소선, 반전), 256/RGB 컬러, 커서 이동, 화면 지우기
- **스크롤백** — 10,000줄 히스토리, Cmd+PageUp/PageDown, 시각적 스크롤바
- **마우스 선택 & 클립보드** — 와이드 문자 인식 드래그 선택, Cmd+C/V
- **폰트 줌** — Cmd+=/- 로 크기 조절 (8pt–72pt)
- **박스 드로잉** — 가는 선, 굵은 선, 이중 선, 둥근 모서리 문자의 기하학적 렌더링
- **키보드** — xterm 스타일 인코딩, Shift/Ctrl/Alt 조합키 지원

## 아키텍처

```
키 입력 → 입력 인코딩 → PTY
                         ↓
                      VT 파서
                         ↓
                        그리드
                         ↓
                    렌더 커맨드
                         ↓
                     GPU 렌더링 → 화면
```

| 모듈 | 역할 |
|---|---|
| 공유 타입 | 핵심 데이터 타입 (`Cell`, `Color`, `RenderCommand`, `TerminalCommand`, `KeyEvent` 등). 모든 모듈이 공유하는 공통 어휘. |
| 입력 인코딩 | `KeyEvent`(키 + 수정자)를 xterm 이스케이프 코드로 PTY 호환 바이트 시퀀스로 변환. 사용자 키보드 입력과 셸 프로세스 사이를 연결. |
| PTY | 의사 터미널에서 셸 프로세스를 생성하고 관리 (`PtyReader`, `PtyWriter`, `PtyResponder`). 양방향 I/O 브릿지 — 인코딩된 입력을 셸에 보내고, 셸의 원시 출력 바이트를 수신. |
| VT 파서 | `vte` 크레이트로 터미널 출력 바이트를 파싱해 `TerminalCommand`(출력, 커서 이동, SGR 속성)를 생성. 불투명한 바이트 스트림을 구조화된 명령으로 변환. |
| 그리드 | 커서 위치, 스크롤백 히스토리, 현재 스타일 상태를 유지하는 2D 셀 버퍼. `TerminalCommand`를 순차 적용해 그리드 상태를 변경하고, `visible_cells()`로 렌더링에 노출. |
| 렌더 커맨드 | 그리드 셀 + 커서/선택/프리에딧 오버레이를 `RenderCommand`로 변환하고 색상을 RGB로 해석. 최종 CPU 단계 준비 — 모든 셀이 위치, 문자, 색상, 플래그를 가진 드로우 명령이 됨. |
| GPU 렌더링 | `GlyphAtlas`가 문자를 비트맵으로 래스터라이즈하고, `GpuDrawer`가 `RenderCommand`를 Metal로 렌더링. GPU 가속을 활용해 실제 화면 출력을 수행. |
| macOS | 네이티브 macOS 통합: 윈도우 생명주기, IME 입력, 이벤트 처리, 앱 델리게이트. 네이티브 이벤트를 앱 레이어로 연결하는 플랫폼 계층. |
| 앱 | 메인 이벤트 루프 코디네이터: PTY 출력 읽기, 이벤트 디스패치, 전체 그리드 → 렌더 파이프라인 조율. 모든 컴포넌트를 동기화하고 타이밍(프레임 레이트, 리사이즈, 입력 전달)을 관리. |

## 빌드 & 실행

```bash
cargo build --release
cargo run -p growterm-app
```

### macOS 앱으로 설치

```bash
./install.sh
```

릴리스 바이너리를 빌드하고 `growTerm.app`을 `/Applications`에 설치한다.

## 테스트

```bash
cargo test
```

258개 이상 테스트 (단위 + 통합).

## 요구사항

- Rust (stable)
- macOS (wgpu Metal 백엔드)
