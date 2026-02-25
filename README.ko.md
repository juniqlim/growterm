<p align="center">
  <img src="assets/banner.svg" alt="growTerm banner" width="100%"/>
</p>

[English](README.md)

자라나는 터미널 앱 — Rust로 만든 GPU 가속 터미널 에뮬레이터. macOS 지원.

## 설계 목표

- **Modular**: 각 모듈은 하나의 책임만 갖는다. 클립보드 복사를 고칠 때 VT 파서를 몰라도 된다.
- **Testable**: 순수 함수와 상태 머신은 단위 테스트로, 모듈 간 연동은 통합 테스트로 검증한다.
- **Evolvable**: 새 기능을 추가할 때 기존 코드를 다시 짜는 게 아니라 새 모듈을 추가한다.

## 특징

- **GPU 렌더링** — wgpu 기반 2-pass 렌더링 (배경 + 글리프)
- **한글 지원** — IME 입력, 와이드 문자 처리, D2Coding 폰트
- **VT 파싱** — SGR 속성, 256/RGB 컬러, 커서 이동, 화면 지우기
- **마우스 선택 & 클립보드** — 드래그 선택, Cmd+C/V
- **폰트 줌** — Cmd+=/- 로 크기 조절
- **블록/박스 드로잉 문자** — 기하학적 렌더링

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
| 공유 타입 | Cell, Color, RenderCommand 등 |
| VT 파서 | VT100/xterm 이스케이프 시퀀스 파싱 |
| 그리드 | 터미널 그리드 상태 관리 |
| 렌더 커맨드 | 그리드 → 렌더 커맨드 변환 |
| GPU 렌더링 | wgpu 기반 화면 출력 |
| 입력 인코딩 | 키 입력 → PTY 바이트 변환 |
| PTY | 셸 프로세스 관리 |
| 앱 | 이벤트 루프, 모듈 통합 |

## 빌드 & 실행

```bash
cargo build --release
cargo run -p growterm-app
```

## 테스트

```bash
cargo test
```

258개 이상 테스트 (단위 + 통합).

## 요구사항

- Rust (stable)
- macOS (wgpu Metal 백엔드)
