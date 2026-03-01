use std::time::{Duration, Instant};

const COMPLETION_TIMEOUT: Duration = Duration::from_millis(500);
const MIN_DURATION_FOR_AVG: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq)]
enum State {
    Idle,
    WaitingForFirstByte,
    Receiving,
}

pub struct ResponseTimer {
    enabled: bool,
    state: State,
    enter_at: Option<Instant>,
    first_byte_at: Option<Instant>,
    last_output_at: Option<Instant>,
    last_ttfb: Option<Duration>,
    last_total: Option<Duration>,
    total_sum: Duration,
    count: u32,
}

impl ResponseTimer {
    pub fn new() -> Self {
        Self {
            enabled: false,
            state: State::Idle,
            enter_at: None,
            first_byte_at: None,
            last_output_at: None,
            last_ttfb: None,
            last_total: None,
            total_sum: Duration::ZERO,
            count: 0,
        }
    }

    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
        if !self.enabled {
            self.state = State::Idle;
            self.enter_at = None;
            self.first_byte_at = None;
            self.last_output_at = None;
            self.last_ttfb = None;
            self.last_total = None;
            self.total_sum = Duration::ZERO;
            self.count = 0;
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn on_enter(&mut self) {
        self.on_enter_at(Instant::now());
    }

    fn on_enter_at(&mut self, now: Instant) {
        if !self.enabled {
            return;
        }
        self.state = State::WaitingForFirstByte;
        self.enter_at = Some(now);
        self.first_byte_at = None;
        self.last_output_at = None;
    }

    pub fn on_pty_output(&mut self, ts: Instant) {
        if !self.enabled {
            return;
        }
        match self.state {
            State::WaitingForFirstByte => {
                self.first_byte_at = Some(ts);
                self.last_output_at = Some(ts);
                if let Some(enter) = self.enter_at {
                    self.last_ttfb = Some(ts.duration_since(enter));
                }
                self.state = State::Receiving;
            }
            State::Receiving => {
                self.last_output_at = Some(ts);
            }
            State::Idle => {}
        }
    }

    pub fn tick(&mut self) {
        self.tick_at(Instant::now());
    }

    fn tick_at(&mut self, now: Instant) {
        if !self.enabled {
            return;
        }
        if self.state == State::Receiving {
            if let Some(last) = self.last_output_at {
                if now.duration_since(last) >= COMPLETION_TIMEOUT {
                    // Response complete
                    if let Some(enter) = self.enter_at {
                        let total = last.duration_since(enter);
                        self.last_total = Some(total);
                        if total >= MIN_DURATION_FOR_AVG {
                            self.total_sum += total;
                            self.count += 1;
                        }
                    }
                    self.state = State::Idle;
                }
            }
        }
    }

    pub fn stats(&self) -> (Duration, u32) {
        (self.total_sum, self.count)
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        if enabled && !self.enabled {
            self.enabled = true;
        } else if !enabled && self.enabled {
            self.toggle();
        }
    }

    pub fn display_text(&self) -> Option<String> {
        self.display_text_at(Instant::now())
    }

    fn display_text_at(&self, now: Instant) -> Option<String> {
        if !self.enabled {
            return None;
        }
        let avg_part = if self.count > 0 {
            format!(" ({}s/{})", (self.total_sum / self.count).as_secs(), self.count)
        } else {
            String::new()
        };
        match self.state {
            State::WaitingForFirstByte | State::Receiving => {
                let elapsed = self.enter_at.map(|e| now.duration_since(e))?;
                Some(format!("⏱ {}s{}", elapsed.as_secs(), avg_part))
            }
            State::Idle => {
                let total = self.last_total?;
                Some(format!("⏱ {}s{}", total.as_secs(), avg_part))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enabled_timer() -> ResponseTimer {
        let mut rt = ResponseTimer::new();
        rt.toggle();
        rt
    }

    #[test]
    fn initial_state_is_disabled_no_display() {
        let rt = ResponseTimer::new();
        assert!(!rt.is_enabled());
        assert!(rt.display_text_at(Instant::now()).is_none());
    }

    #[test]
    fn toggle_enables_and_disables() {
        let mut rt = ResponseTimer::new();
        assert!(!rt.is_enabled());
        rt.toggle();
        assert!(rt.is_enabled());
        rt.toggle();
        assert!(!rt.is_enabled());
    }

    #[test]
    fn toggle_off_resets_state() {
        let mut rt = enabled_timer();
        let now = Instant::now();
        rt.on_enter_at(now);
        assert_eq!(rt.state, State::WaitingForFirstByte);
        rt.toggle();
        assert_eq!(rt.state, State::Idle);
        assert!(rt.last_ttfb.is_none());
    }

    #[test]
    fn on_enter_ignored_when_disabled() {
        let mut rt = ResponseTimer::new();
        rt.on_enter_at(Instant::now());
        assert_eq!(rt.state, State::Idle);
    }

    #[test]
    fn on_enter_transitions_to_waiting() {
        let mut rt = enabled_timer();
        let now = Instant::now();
        rt.on_enter_at(now);
        assert_eq!(rt.state, State::WaitingForFirstByte);
    }

    #[test]
    fn waiting_display_shows_elapsed() {
        let mut rt = enabled_timer();
        let now = Instant::now();
        rt.on_enter_at(now);
        let text = rt
            .display_text_at(now + Duration::from_secs(2))
            .unwrap();
        assert_eq!(text, "⏱ 2s");
    }

    #[test]
    fn waiting_display_shows_prev_avg() {
        let mut rt = enabled_timer();
        let now = Instant::now();
        // Complete first command (3s)
        rt.on_enter_at(now);
        rt.on_pty_output(now + Duration::from_secs(3));
        rt.tick_at(now + Duration::from_secs(4));

        // Start second command
        rt.on_enter_at(now + Duration::from_secs(10));
        let text = rt
            .display_text_at(now + Duration::from_secs(12))
            .unwrap();
        assert_eq!(text, "⏱ 2s (3s/1)");
    }

    #[test]
    fn on_pty_output_records_ttfb() {
        let mut rt = enabled_timer();
        let now = Instant::now();
        rt.on_enter_at(now);
        rt.on_pty_output(now + Duration::from_millis(50));
        assert_eq!(rt.state, State::Receiving);
        assert_eq!(rt.last_ttfb, Some(Duration::from_millis(50)));
    }

    #[test]
    fn receiving_display_shows_elapsed() {
        let mut rt = enabled_timer();
        let now = Instant::now();
        rt.on_enter_at(now);
        rt.on_pty_output(now + Duration::from_millis(50));
        let text = rt
            .display_text_at(now + Duration::from_secs(5))
            .unwrap();
        assert_eq!(text, "⏱ 5s");
    }

    #[test]
    fn tick_completes_after_500ms_silence() {
        let mut rt = enabled_timer();
        let now = Instant::now();
        rt.on_enter_at(now);
        rt.on_pty_output(now + Duration::from_millis(50));
        rt.on_pty_output(now + Duration::from_secs(3));

        rt.tick_at(now + Duration::from_millis(3499));
        assert_eq!(rt.state, State::Receiving);

        rt.tick_at(now + Duration::from_millis(3500));
        assert_eq!(rt.state, State::Idle);
        assert_eq!(rt.last_total, Some(Duration::from_secs(3)));
    }

    #[test]
    fn idle_after_completion_shows_total_and_avg() {
        let mut rt = enabled_timer();
        let now = Instant::now();
        rt.on_enter_at(now);
        rt.on_pty_output(now + Duration::from_secs(5));
        rt.tick_at(now + Duration::from_millis(5500));

        let text = rt
            .display_text_at(now + Duration::from_secs(10))
            .unwrap();
        assert_eq!(text, "⏱ 5s (5s/1)");
    }

    #[test]
    fn session_average_across_multiple_commands() {
        let mut rt = enabled_timer();
        let now = Instant::now();

        // First command: total 2s
        rt.on_enter_at(now);
        rt.on_pty_output(now + Duration::from_secs(2));
        rt.tick_at(now + Duration::from_millis(2500));
        assert_eq!(rt.count, 1);

        // Second command: total 4s
        let t2 = now + Duration::from_secs(10);
        rt.on_enter_at(t2);
        rt.on_pty_output(t2 + Duration::from_secs(4));
        rt.tick_at(t2 + Duration::from_millis(4500));
        assert_eq!(rt.count, 2);

        // avg = (2+4)/2 = 3s
        let text = rt
            .display_text_at(t2 + Duration::from_secs(10))
            .unwrap();
        assert_eq!(text, "⏱ 4s (3s/2)");
    }

    #[test]
    fn stats_returns_total_sum_and_count() {
        let mut rt = enabled_timer();
        let now = Instant::now();

        // First command: 2s
        rt.on_enter_at(now);
        rt.on_pty_output(now + Duration::from_secs(2));
        rt.tick_at(now + Duration::from_millis(2500));

        // Second command: 4s
        let t2 = now + Duration::from_secs(10);
        rt.on_enter_at(t2);
        rt.on_pty_output(t2 + Duration::from_secs(4));
        rt.tick_at(t2 + Duration::from_millis(4500));

        let (total_sum, count) = rt.stats();
        assert_eq!(total_sum, Duration::from_secs(6));
        assert_eq!(count, 2);
    }

    #[test]
    fn set_enabled_enables_and_disables() {
        let mut rt = ResponseTimer::new();
        rt.set_enabled(true);
        assert!(rt.is_enabled());
        rt.set_enabled(false);
        assert!(!rt.is_enabled());
        // Double enable is no-op
        rt.set_enabled(true);
        rt.set_enabled(true);
        assert!(rt.is_enabled());
    }

    #[test]
    fn on_pty_output_ignored_when_idle() {
        let mut rt = enabled_timer();
        rt.on_pty_output(Instant::now());
        assert_eq!(rt.state, State::Idle);
    }

    #[test]
    fn sub_second_response_excluded_from_avg() {
        let mut rt = enabled_timer();
        let now = Instant::now();

        // Fast command: 50ms (below 1s threshold)
        rt.on_enter_at(now);
        rt.on_pty_output(now + Duration::from_millis(50));
        rt.tick_at(now + Duration::from_millis(550));
        assert_eq!(rt.state, State::Idle);
        assert_eq!(rt.last_total, Some(Duration::from_millis(50)));
        assert_eq!(rt.count, 0); // not counted in avg

        // Slow command: 5s
        let t2 = now + Duration::from_secs(5);
        rt.on_enter_at(t2);
        rt.on_pty_output(t2 + Duration::from_secs(5));
        rt.tick_at(t2 + Duration::from_millis(5500));
        assert_eq!(rt.count, 1);

        // avg should be 5s (fast command excluded)
        let text = rt
            .display_text_at(t2 + Duration::from_secs(10))
            .unwrap();
        assert_eq!(text, "⏱ 5s (5s/1)");
    }

    #[test]
    fn new_enter_resets_in_flight_state() {
        let mut rt = enabled_timer();
        let now = Instant::now();
        rt.on_enter_at(now);
        rt.on_pty_output(now + Duration::from_millis(50));

        rt.on_enter_at(now + Duration::from_millis(100));
        assert_eq!(rt.state, State::WaitingForFirstByte);
        assert!(rt.first_byte_at.is_none());
    }
}
