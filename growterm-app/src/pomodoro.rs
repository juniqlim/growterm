use std::time::Instant;

const WORK_SECS: u64 = 25 * 60;
const BREAK_SECS: u64 = 3 * 60;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Phase {
    Idle,
    Working,
    Break,
}

pub struct Pomodoro {
    enabled: bool,
    phase: Phase,
    started_at: Option<Instant>,
}

impl Pomodoro {
    pub fn new() -> Self {
        Self {
            enabled: false,
            phase: Phase::Idle,
            started_at: None,
        }
    }

    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
        if !self.enabled {
            self.phase = Phase::Idle;
            self.started_at = None;
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    #[cfg(test)]
    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// Called when user types. Starts work timer if idle.
    pub fn on_input(&mut self) {
        self.on_input_at(Instant::now());
    }

    fn on_input_at(&mut self, now: Instant) {
        if !self.enabled {
            return;
        }
        if self.phase == Phase::Idle {
            self.phase = Phase::Working;
            self.started_at = Some(now);
        }
    }

    /// Called periodically. Transitions state if time elapsed.
    pub fn tick(&mut self) {
        self.tick_at(Instant::now());
    }

    fn tick_at(&mut self, now: Instant) {
        if !self.enabled {
            return;
        }
        let started = match self.started_at {
            Some(t) => t,
            None => return,
        };
        let elapsed = now.duration_since(started).as_secs();
        match self.phase {
            Phase::Working => {
                if elapsed >= WORK_SECS {
                    self.phase = Phase::Break;
                    self.started_at = Some(now);
                }
            }
            Phase::Break => {
                if elapsed >= BREAK_SECS {
                    self.phase = Phase::Idle;
                    self.started_at = None;
                }
            }
            Phase::Idle => {}
        }
    }

    pub fn is_input_blocked(&self) -> bool {
        self.enabled && self.phase == Phase::Break
    }

    /// Returns display text for the timer, or None if idle.
    pub fn display_text(&self) -> Option<String> {
        self.display_text_at(Instant::now())
    }

    fn display_text_at(&self, now: Instant) -> Option<String> {
        if !self.enabled {
            return None;
        }
        let started = self.started_at?;
        let elapsed = now.duration_since(started).as_secs();
        match self.phase {
            Phase::Working => {
                let remaining = WORK_SECS.saturating_sub(elapsed);
                let m = remaining / 60;
                let s = remaining % 60;
                Some(format!("\u{1F345} {m:02}:{s:02}"))
            }
            Phase::Break => {
                let remaining = BREAK_SECS.saturating_sub(elapsed);
                let m = remaining / 60;
                let s = remaining % 60;
                Some(format!("\u{2615} {m:02}:{s:02}"))
            }
            Phase::Idle => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn enabled_pomodoro() -> Pomodoro {
        let mut p = Pomodoro::new();
        p.toggle();
        p
    }

    #[test]
    fn initial_state_is_disabled() {
        let p = Pomodoro::new();
        assert!(!p.is_enabled());
        assert!(!p.is_input_blocked());
        assert!(p.display_text_at(Instant::now()).is_none());
    }

    #[test]
    fn toggle_enables_and_disables() {
        let mut p = Pomodoro::new();
        assert!(!p.is_enabled());
        p.toggle();
        assert!(p.is_enabled());
        p.toggle();
        assert!(!p.is_enabled());
    }

    #[test]
    fn toggle_off_resets_state() {
        let mut p = enabled_pomodoro();
        let now = Instant::now();
        p.on_input_at(now);
        assert_eq!(p.phase(), Phase::Working);

        p.toggle(); // disable
        assert_eq!(p.phase(), Phase::Idle);
        assert!(!p.is_input_blocked());
    }

    #[test]
    fn on_input_ignored_when_disabled() {
        let mut p = Pomodoro::new(); // disabled
        let now = Instant::now();
        p.on_input_at(now);
        assert_eq!(p.phase(), Phase::Idle);
    }

    #[test]
    fn on_input_starts_working() {
        let mut p = enabled_pomodoro();
        let now = Instant::now();
        p.on_input_at(now);
        assert_eq!(p.phase(), Phase::Working);
        assert!(!p.is_input_blocked());
    }

    #[test]
    fn on_input_during_working_is_noop() {
        let mut p = enabled_pomodoro();
        let now = Instant::now();
        p.on_input_at(now);
        let before = p.started_at;
        p.on_input_at(now + Duration::from_secs(10));
        assert_eq!(p.started_at, before);
    }

    #[test]
    fn tick_transitions_working_to_break_after_25min() {
        let mut p = enabled_pomodoro();
        let now = Instant::now();
        p.on_input_at(now);

        p.tick_at(now + Duration::from_secs(WORK_SECS - 1));
        assert_eq!(p.phase(), Phase::Working);

        p.tick_at(now + Duration::from_secs(WORK_SECS));
        assert_eq!(p.phase(), Phase::Break);
        assert!(p.is_input_blocked());
    }

    #[test]
    fn tick_transitions_break_to_idle_after_3min() {
        let mut p = enabled_pomodoro();
        let now = Instant::now();
        p.on_input_at(now);

        let break_start = now + Duration::from_secs(WORK_SECS);
        p.tick_at(break_start);
        assert_eq!(p.phase(), Phase::Break);

        p.tick_at(break_start + Duration::from_secs(BREAK_SECS - 1));
        assert_eq!(p.phase(), Phase::Break);

        p.tick_at(break_start + Duration::from_secs(BREAK_SECS));
        assert_eq!(p.phase(), Phase::Idle);
        assert!(!p.is_input_blocked());
    }

    #[test]
    fn display_text_during_working() {
        let mut p = enabled_pomodoro();
        let now = Instant::now();
        p.on_input_at(now);

        let text = p.display_text_at(now + Duration::from_secs(30)).unwrap();
        assert!(text.starts_with('\u{1F345}')); // 🍅
        assert!(text.contains("24:30"));
    }

    #[test]
    fn display_text_during_break() {
        let mut p = enabled_pomodoro();
        let now = Instant::now();
        p.on_input_at(now);

        let break_start = now + Duration::from_secs(WORK_SECS);
        p.tick_at(break_start);

        let text = p.display_text_at(break_start + Duration::from_secs(15)).unwrap();
        assert!(text.starts_with('\u{2615}')); // ☕
        assert!(text.contains("02:45"));
    }

    #[test]
    fn full_cycle_idle_work_break_idle() {
        let mut p = enabled_pomodoro();
        let now = Instant::now();

        p.on_input_at(now);
        assert_eq!(p.phase(), Phase::Working);

        let t1 = now + Duration::from_secs(WORK_SECS);
        p.tick_at(t1);
        assert_eq!(p.phase(), Phase::Break);

        let t2 = t1 + Duration::from_secs(BREAK_SECS);
        p.tick_at(t2);
        assert_eq!(p.phase(), Phase::Idle);

        p.on_input_at(t2 + Duration::from_secs(1));
        assert_eq!(p.phase(), Phase::Working);
    }
}
