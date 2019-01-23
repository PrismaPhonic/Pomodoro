use std::thread::sleep;
use std::time::{Duration, SystemTime};

#[derive(Debug)]
pub struct StateTracker {
    current_order: Option<i32>,
    current_state: PomodoroState,
    started_at: Option<SystemTime>,
}

impl StateTracker {
    pub fn new() -> StateTracker {
        StateTracker {
            current_order: None,
            current_state: PomodoroState::None,
            started_at: None,
        }
    }

    fn increment_cycle(&mut self) {
        let new_order = match self.current_order {
            Some(num) if num < 4 => Some(num + 1),
            None => Some(1),
            _ => None,
        };
        self.current_order = new_order;
    }

    fn restart_cycle(&mut self) {
        self.current_order = None;
    }

    pub fn get_order(&self) -> Option<i32> {
        self.current_order
    }

    pub fn start_work(&mut self) {
        let now = SystemTime::now();
        self.started_at = Some(now);

        let mut clock = Clock::new();
        self.current_state = PomodoroState::Working;
        self.increment_cycle();
        clock.set_time_minutes(25);
        clock.countdown();
        self.set_break();
        self.start_break();
    }

    pub fn start_break(&mut self) {
        match self.current_state {
            PomodoroState::ShortBreak => self.short_break(),
            PomodoroState::LongBreak => self.long_break(),
            _ => (),
        }
    }

    pub fn short_break(&mut self) {
        let mut clock = Clock::new();
        clock.set_time_minutes(5);
        clock.countdown();
    }

    pub fn long_break(&mut self) {
        let mut clock = Clock::new();
        clock.set_time_minutes(30);
        clock.countdown();
    }

    pub fn set_break(&mut self) {
        let break_state = match self.current_order {
            Some(_x @ 0..=3) => PomodoroState::ShortBreak,
            Some(_x @ 4) => PomodoroState::LongBreak,
            Some(_) => PomodoroState::None,
            None => PomodoroState::None,
        };

        self.current_state = break_state;
    }
}

#[derive(Debug)]
enum PomodoroState {
    Working,
    ShortBreak,
    LongBreak,
    None,
}

struct Clock {
    minutes: u32,
    seconds: u32,
}

impl Clock {
    pub fn new() -> Clock {
        Clock {
            minutes: 0,
            seconds: 0,
        }
    }

    pub fn set_time_ms(&mut self, ms: u32) {
        self.minutes = (ms / (1000 * 60)) % 60;
        self.seconds = (ms / 1000) % 60;
    }

    pub fn set_time_minutes(&mut self, minutes: u32) {
        self.set_time_ms(minutes * 60000);
    }

    pub fn decrement_one_second(&mut self) {
        let mut time_in_ms = self.get_ms_from_time();
        time_in_ms -= 1000;
        self.set_time_ms(time_in_ms);
    }

    pub fn get_ms_from_time(&mut self) -> u32 {
        (self.minutes * 60000) + (self.seconds * 1000)
    }

    pub fn get_time(&self) -> String {
        format!("{:02}:{:02}", self.minutes, self.seconds)
    }

    pub fn countdown(&mut self) {
        loop {
            sleep(Duration::new(1, 0));
            self.decrement_one_second();
            let current_clock = self.get_time();
            if current_clock == "00:00".to_string() {
                break;
            }
            println!("{}", self.get_time())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clock_ms() {
        let mut clock = Clock::new();
        clock.set_time_ms(60000);
        assert_eq!(clock.get_time(), "01:00");
    }

    #[test]
    fn test_clock_minutes() {
        let mut clock = Clock::new();
        clock.set_time_minutes(25);
        assert_eq!(clock.get_time(), "25:00");
    }

    #[test]
    fn test_start_cycle() {
        let mut pstate = StateTracker::new();
        pstate.increment_cycle();
        assert_eq!(pstate.get_order().unwrap(), 1);
    }

    #[test]
    fn test_increment_cycle() {
        let mut pstate = StateTracker::new();
        pstate.increment_cycle();
        pstate.increment_cycle();
        assert_eq!(pstate.get_order().unwrap(), 2);
    }

    #[test]
    fn test_cycle_loop() {
        let mut pstate = StateTracker::new();
        pstate.increment_cycle();
        pstate.increment_cycle();
        pstate.increment_cycle();
        pstate.increment_cycle();
        pstate.increment_cycle();
        assert_eq!(pstate.get_order(), None);
    }

    #[test]
    fn test_cycle_restart() {
        let mut pstate = StateTracker::new();
        pstate.restart_cycle();
        assert_eq!(pstate.get_order(), None);
    }
}
