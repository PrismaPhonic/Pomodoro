#[macro_use]
extern crate structopt;

use std::io;
use std::io::{Read, Write};
use std::thread::sleep;
use std::time::{Duration, Instant};

use std::error::Error;
use termion::raw::IntoRawMode;
use termion::{clear, cursor, style};

use notify_rust::Notification;

/// The pomodoro menu.
const POMODORO_MENU: &'static str = "\r\n
╔═════════════════╗
║───┬ Pomodoro────║
║ s ┆ start       ║
║ q ┆ quit        ║
╚═══╧═════════════╝";

/// Pinging sound when clock is up
#[cfg(target_os = "macos")]
static SOUND: &'static str = "Ping";

#[cfg(all(unix, not(target_0s = "macos")))]
static SOUND: &'static str = "alarm-clock-elapsed";

/**
 * Terminal flag settings
 */
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "pomodoro", about = "a rust based pomodoro timer")]
/// You can use this terminal program to start a pomodoro timer
pub struct PomodoroConfig {
    #[structopt(short = "w", long = "work", default_value = "25")]
    /// Sets length of work period in minutes
    work: u64,

    #[structopt(short = "s", long = "shortbreak", default_value = "5")]
    /// Sets length of your short break in minutes
    short_break: u64,

    #[structopt(short = "l", long = "longbreak", default_value = "20")]
    /// Sets length of your long break in minutes
    long_break: u64,
}

pub struct PomodoroSession<R, W> {
    stdin: R,
    stdout: W,
    width: u16,
    height: u16,
    pomodoro_tracker: StateTracker,
    clock: Clock,
    config: PomodoroConfig,
}

impl<R: Read, W: Write> PomodoroSession<R, W> {
    fn start(&mut self) {
        write!(self.stdout, "{}", cursor::Hide).unwrap();
        self.display_menu();
    }

    fn begin_cycle(&mut self) {
        self.start_work();
        self.display_menu();
    }

    pub fn start_work(&mut self) {
        self.pomodoro_tracker.set_work_state();
        self.clock.set_time_minutes(25);
        self.countdown();
    }

    pub fn countdown(&mut self) {
        match self.pomodoro_tracker.current_state {
            PomodoroState::Working => self.countdown_work(),
            PomodoroState::ShortBreak | PomodoroState::LongBreak => {
                self.countdown_break();
            }
            _ => (),
        }
    }

    pub fn countdown_work(&mut self) {
        loop {
            let elapsed: u64 = (self
                .pomodoro_tracker
                .started_at
                .unwrap()
                .elapsed()
                .as_millis()) as u64;

            // take work time in milliseconds and subtract from current clock time
            // in milliseconds to get the current elapsed "clock time" - then
            // correct any errors from actual elapsed time and add 1 second to
            // sleep to sync our display clock
            let current = (self.config.work * 60_000) - self.clock.get_ms_from_time();

            let sync_offset = elapsed - current;

            sleep(Duration::from_millis(1000 - sync_offset));

            if let Command::Quit = self.async_command_listen() {
                return;
            }

            self.clock.decrement_one_second();
            self.draw_work_screen();

            if &self.clock.get_ms_from_time() == &0 {
                break;
            }
        }
        Notification::new()
            .summary("Pomodoro Break!")
            .body("It's Time For a Break!")
            .appname("Pomodoro")
            .sound_name(SOUND)
            .icon("clock")
            .show()
            .unwrap();
        self.pomodoro_tracker.set_break_state();
        self.start_break();
    }

    pub fn start_break(&mut self) {
        match self.pomodoro_tracker.current_state {
            PomodoroState::ShortBreak => self.short_break(),
            PomodoroState::LongBreak => self.long_break(),
            _ => (),
        }
    }

    pub fn short_break(&mut self) {
        self.clock.set_time_minutes(1);
        self.countdown();
    }

    pub fn long_break(&mut self) {
        self.clock.set_time_minutes(20);
        self.countdown();
    }

    pub fn countdown_break(&mut self) {
        loop {
            sleep(Duration::new(1, 0));

            if let Command::Quit = self.async_command_listen() {
                break;
            }

            self.clock.decrement_one_second();
            self.draw_rest_screen();

            if &self.clock.get_ms_from_time() == &0 {
                break;
            }
        }
        Notification::new()
            .summary("Pomodoro Break Over")
            .body("Ready for Another Round?")
            .appname("Pomodoro")
            .sound_name(SOUND)
            .icon("clock")
            .show()
            .unwrap();
    }

    /**
     * CLOCK AND DRAWING METHODS
     */

    pub fn draw_work_screen(&mut self) {
        let clock = self.clock.gen_clock("Time to Work!");
        self.draw_work_count();
        self.draw_clock(clock);
    }

    pub fn draw_rest_screen(&mut self) {
        let clock = self.clock.gen_clock("Time to Chill");
        self.draw_work_count();
        self.draw_clock(clock);
    }

    pub fn draw_clock(&mut self, clock: String) {
        for (i, line) in clock.lines().enumerate() {
            write!(
                self.stdout,
                "{}{}{}",
                cursor::Goto((&self.width / 2) - 20, (&self.height / 2) - 3 + i as u16),
                clear::CurrentLine,
                line
            )
            .unwrap();
        }
    }

    pub fn draw_work_count(&mut self) {
        write!(
            self.stdout,
            "\r\n{}{}Work Period {} of 4",
            cursor::Goto((&self.width / 2) - 8, (&self.height / 2) + 5),
            clear::CurrentLine,
            &self.pomodoro_tracker.current_order.unwrap(),
        )
        .unwrap();
    }

    pub fn display_menu(&mut self) {
        let lines = POMODORO_MENU.lines();
        let mut last_i = 0;
        for (i, line) in lines.enumerate() {
            write!(
                self.stdout,
                "{}{}{}",
                cursor::Goto((&self.width / 2) - 9, (&self.height / 2) - 3 + i as u16),
                clear::CurrentLine,
                line,
            )
            .unwrap();
            last_i = i;
        }

        // clear 4 lines below also so it clears out ascii left over from clock
        for i in last_i + 1..last_i + 4 {
            write!(
                self.stdout,
                "{}{}",
                cursor::Goto((&self.width / 2) - 9, (&self.height / 2) - 3 + i as u16),
                clear::CurrentLine,
            )
            .unwrap();
        }

        self.stdout.flush().unwrap();

        match self.wait_for_next_command() {
            Command::Start => self.begin_cycle(),
            Command::Quit => return,
            Command::Stop => (),
            Command::Restart => (),
            Command::Reset => (),
            Command::None => (),
        }
    }

    pub fn wait_for_next_command(&mut self) -> Command {
        let mut command = Command::None;

        while let Command::None = command {
            let mut buf = [0];
            self.stdin.read(&mut buf).unwrap();
            command = match buf[0] {
                b's' => Command::Start,
                b'x' => Command::Stop,
                b'r' => Command::Restart,
                b'q' => Command::Quit,
                _ => continue,
            }
        }

        command
    }

    pub fn async_command_listen(&mut self) -> Command {
        let mut buf = [0];
        self.stdin.read(&mut buf).unwrap();
        let command = match buf[0] {
            b'x' => Command::Stop,
            b'r' => Command::Restart,
            b'q' => Command::Quit,
            _ => Command::None,
        };

        command
    }
}

#[derive(Debug)]
pub struct StateTracker {
    current_order: Option<i32>,
    current_state: PomodoroState,
    started_at: Option<Instant>,
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
            _ => Some(1),
        };
        self.current_order = new_order;
    }

    pub fn get_order(&self) -> Option<i32> {
        self.current_order
    }

    pub fn set_work_state(&mut self) {
        let now = Instant::now();
        self.started_at = Some(now);

        self.current_state = PomodoroState::Working;
        self.increment_cycle();
    }

    pub fn set_break_state(&mut self) {
        let break_state = match self.current_order {
            Some(_x @ 0..=3) => PomodoroState::ShortBreak,
            Some(_x @ 4) => PomodoroState::LongBreak,
            Some(_) => PomodoroState::None,
            None => PomodoroState::None,
        };

        self.current_state = break_state;
    }
}

pub enum Command {
    Start,
    Stop,
    Restart,
    Reset,
    Quit,
    None,
}

#[derive(Debug)]
enum PomodoroState {
    Working,
    ShortBreak,
    LongBreak,
    None,
}

struct Clock {
    minutes: u64,
    seconds: u64,
}

impl Clock {
    pub fn new() -> Clock {
        Clock {
            minutes: 0,
            seconds: 0,
        }
    }

    pub fn set_time_ms(&mut self, ms: u64) {
        self.minutes = (ms / (1000 * 60)) % 60;
        self.seconds = (ms / 1000) % 60;
    }

    pub fn set_time_minutes(&mut self, minutes: u64) {
        self.set_time_ms(minutes * 60000);
    }

    pub fn decrement_one_second(&mut self) {
        let mut time_in_ms = self.get_ms_from_time();
        time_in_ms -= 1000;
        self.set_time_ms(time_in_ms);
    }

    pub fn get_ms_from_time(&mut self) -> u64 {
        (self.minutes * 60000) + (self.seconds * 1000)
    }

    pub fn get_time(&self) -> String {
        format!("{:02}:{:02}", self.minutes, self.seconds)
    }

    pub fn gen_clock(&self, message: &str) -> String {
        let clock = format!("\r\n
╭───────────────────────────────────────╮
│                                       │
│             {}             │
│                 {}                 │
│                                       │
╰───────────────────────────────────────╯
", message, self.get_time());
        clock
    }
}

fn init(width: u16, height: u16, config: PomodoroConfig) {
    let stdout = io::stdout();
    let mut stdout = stdout.lock().into_raw_mode().unwrap();
    let stdin = termion::async_stdin();

    write!(stdout, "{}{}", clear::All, cursor::Goto(1, 1)).unwrap();
    stdout.flush().unwrap();

    let mut pomodoro_screen = PomodoroSession {
        width: width,
        height: height,
        stdin: stdin,
        stdout: stdout,
        pomodoro_tracker: StateTracker::new(),
        clock: Clock::new(),
        config,
    };

    write!(
        pomodoro_screen.stdout,
        "{}{}",
        clear::All,
        cursor::Goto(1, 1)
    )
    .unwrap();

    pomodoro_screen.start();

    write!(
        pomodoro_screen.stdout,
        "{}{}{}{}",
        clear::All,
        style::Reset,
        cursor::Goto(1, 1),
        cursor::Show,
    )
    .unwrap();
    pomodoro_screen.stdout.flush().unwrap();
}

pub fn run(config: PomodoroConfig) -> Result<(), Box<dyn Error>> {
    let (x, y) = termion::terminal_size().unwrap();
    init(x, y, config);

    Ok(())
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
        clock.set_time_minutes(1);
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
