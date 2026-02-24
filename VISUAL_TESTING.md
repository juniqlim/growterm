# juniqterm 검증 방법

## 테스트 실행

```bash
# 전체 workspace 테스트
cargo test --workspace

# 개별 크레이트 테스트
cargo test -p juniqterm-types
cargo test -p juniqterm-gpu-draw
cargo test -p juniqterm-render-cmd
cargo test -p juniqterm-vt-parser
```

## 시각 확인 (integration examples)

```bash
# Phase 1+2: Cell → generate() → draw()
cargo run --example integration_render_cmd -p juniqterm-gpu-draw

# Phase 1+2+3: bytes → VtParser → Grid → generate() → draw()
cargo run --example integration_vt_parser -p juniqterm-gpu-draw

# Phase 1 단독: 하드코딩 그리드
cargo run --example hardcoded_grid -p juniqterm-gpu-draw
```

## 스크린샷 자동 캡처 (Claude Code용)

사진 첨부 없이 GUI 창을 직접 확인하는 방법.

## 전제조건
- iTerm2 (또는 실행 터미널)에 **손쉬운 사용(Accessibility)** 권한 필요
- 시스템 설정 → 개인 정보 보호 및 보안 → 손쉬운 사용

## 방법

```bash
# 1. 예제를 백그라운드 실행
cargo run --example <NAME> -p <CRATE> 2>&1 &
PID=$!
sleep 3

# 2. 창을 앞으로 가져옴
osascript -e 'tell application "System Events" to set frontmost of (first process whose name contains "<PROCESS_NAME>") to true'
sleep 1

# 3. Swift로 윈도우 ID 취득 (CGWindowListCopyWindowInfo)
WINID=$(swift -e '
import Cocoa
let list = CGWindowListCopyWindowInfo(.optionOnScreenOnly, kCGNullWindowID) as! [[String: Any]]
for w in list {
    if let name = w["kCGWindowOwnerName"] as? String, name.contains("<PROCESS_NAME>") {
        print(w["kCGWindowNumber"]!)
        break
    }
}
' 2>/dev/null)

# 4. 윈도우 ID로 해당 창만 캡처
screencapture -x -l "$WINID" /tmp/screenshot.png

# 5. 정리
kill $PID 2>/dev/null
```

## 4단계 후 Read 도구로 이미지 확인
```
Read /tmp/screenshot.png
```

## 안 되는 방법들 (참고)
- `osascript -e 'get id of first window'` → 접근성 권한 없으면 실패
- `python3 import Quartz` → 기본 설치에 없음
- `screencapture -R x,y,w,h` → 다른 창과 겹치면 배경이 찍힘
