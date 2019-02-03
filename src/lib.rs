//! # pomodoro
//!
//! This crate offers you a functional terminal based pomodoro clock.
//!
//! # Installation
//!
//! This clock requires being built with nightly because of an experimental feature I used to keep
//! the clock in sync and never fluctuating by more than 1ms.  To install:
//!
//! ```terminal
//! $ cargo +nightly install pomodoro
//! ```
//!
//! # Using pomodoro
//!
//! To use, simply run it. By default it will give you a work time of 25 minutes, short break of 5
//! minutes and a long break of 20 minutes.
//!
//! ```terminal
//! $ pomodoro 
//! ```
//!
//! You can pass it terminal flags to customize the times.  `-w` flag will set the work time, `-s`
//! will set the short break time, and `-l` will set the long break time.  Here's an example that
//! sets up a custom pomodoro with 30 minute work time, 10 minute short break and 25 minute long
//! break:
//!
//! ```terminal
//! $ pomodoro -w 30 -s 10 -l 25
//! ```
//!
//! All of the controls for starting, quitting or resetting a pomodoro are displayed by the
//! pomodoro menu on launch. `s` will start your next pomodoro. `q` will take you back to the
//! menu if you are in a pomodoro, or quit if you are at the menu. `r` will reset the current
//! pomodoro (back to the head of the work cycle and immediately begin countdown).
//!
//! Commands are listened for in an asynchronous and non-blocking fashion.
//!
//! Enjoy!

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
const POMODORO_MENU: &'static str = "
╔═════════════════╗
║───┬ Pomodoro────║
║ s ┆ start next  ║
║ q ┆ quit        ║
╚═══╧═════════════╝";

/// Initial pomodoro welcome menu.
pub const POMODORO_START_PROMPT: &'static str = "
╔══════════════════════════════╗
║──Start your first Pomodoro!─-║
║──────────────────────────────║
║ s ┆ start    Press s         ║
║ q ┆ quit     to start!       ║
║ r ┆ reset                    ║
╚═══╧══════════════════════════╝";

/// Controls layout always on screen when clock is rolling.
pub const CONTROLS: &'static str = "
------controls------
 q    ~ end current
 r    ~ reset
";

/// Pinging sound when clock is up.
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
/// You can use this terminal program to start a pomodoro timer.
pub struct PomodoroConfig {
    #[structopt(short = "w", long = "work", default_value = "25")]
    /// Sets length of work period in minutes.
    work_time: u64,

    #[structopt(short = "s", long = "shortbreak", default_value = "5")]
    /// Sets length of your short break in minutes.
    short_break_time: u64,

    #[structopt(short = "l", long = "longbreak", default_value = "20")]
    /// Sets length of your long break in minutes.
    long_break_time: u64,
}

/// This struct represents a pomodoro session - which is from the start of running the application
/// until you terminate it. Between that time this struct will keep track of the initial height and
/// width of the terminal window when launching pomodoro, and keep a lock on stdin and stdout so we
/// can draw to the screen, and also accept async input from the user. Lastly, this session also
/// holds a state tracker, the clock itself (that gets drawn) and a config file.  The config is
/// passed in by the user.  -w flag will pass in a custom work time, -s will pass in a custom short
/// break time, and -l will pass in a custom long break time.
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
        self.display_menu(Some(POMODORO_START_PROMPT));
    }

    fn begin_cycle(&mut self) {
        self.start_work();
        self.display_menu(None);
    }

    /// Call a start to a work cycle.
    pub fn start_work(&mut self) {
        self.pomodoro_tracker.set_work_state();
        self.clock.set_time_minutes(self.config.work_time);
        self.countdown();
    }

    /// Reset the current pomodoro by decrementing the cycle and re-running start_work.
    pub fn reset_current_pomodoro(&mut self) {
        self.pomodoro_tracker.decrement_cycle();
        self.start_work();
    }

    /// Checks the pomodoro state (Working, ShortBreak, or LongBreak) and runs the appropraite
    /// internal countdown method.
    pub fn countdown(&mut self) {
        match self.pomodoro_tracker.current_state {
            PomodoroState::Working => self.countdown_work(),
            PomodoroState::ShortBreak => {
                self.countdown_break(self.config.short_break_time);
            }
            PomodoroState::LongBreak => {
                self.countdown_break(self.config.long_break_time);
            }
            _ => (),
        }
    }

    /// Countdown count for work - with syncing so we are never more than a ms off from true time.
    pub fn countdown_work(&mut self) {
        loop {
            let true_elapsed: u64 = (self
                .pomodoro_tracker
                .started_at
                .unwrap()
                .elapsed()
                .as_millis()) as u64;

            // take work time in milliseconds and subtract from current clock time
            // in milliseconds to get the current elapsed "clock time" - then
            // correct any errors from actual elapsed time and add 1 second to
            // sleep to sync our display clock
            let clock_elapsed = (self.config.work_time * 60_000) - self.clock.get_ms_from_time();

            let sync_offset = true_elapsed - clock_elapsed;

            sleep(Duration::from_millis(1000 - sync_offset));

            match self.async_command_listen() {
                Command::Quit => return,
                Command::Reset => return self.reset_current_pomodoro(),
                _ => (),
            }

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

    /// Starts a break by matching which break state we are in (short or long) and then running the
    /// appropriate break function.
    pub fn start_break(&mut self) {
        match self.pomodoro_tracker.current_state {
            PomodoroState::ShortBreak => self.short_break(),
            PomodoroState::LongBreak => self.long_break(),
            _ => (),
        }
    }

    /// Sets the break time by referencing the config (flags passed in on start) and then starts
    /// the countdown clock.
    pub fn short_break(&mut self) {
        self.clock.set_time_minutes(self.config.short_break_time);
        self.countdown();
    }

    /// Sets the break time by referencing the config (flags passed in on start) and then starts
    /// the countdown clock.
    pub fn long_break(&mut self) {
        self.clock.set_time_minutes(self.config.long_break_time);
        self.countdown();
    }

    /// Countdown clock for a break - extremely similar to countdown-work - separate because the
    /// notifications after the loops are different.  Good place for a refactor.
    pub fn countdown_break(&mut self, duration: u64) {
        loop {
            let true_elapsed: u64 = (self
                .pomodoro_tracker
                .started_at
                .unwrap()
                .elapsed()
                .as_millis()) as u64;

            // take break time in milliseconds and subtract from current clock time
            // in milliseconds + work time in ms to get the current elapsed
            // "clock time" - then correct any errors from actual elapsed time and
            // add 1 second to sleep to sync our display clock
            let clock_elapsed = (duration * 60_000) - self.clock.get_ms_from_time()
                + (self.config.work_time * 60_000);

            let sync_offset = true_elapsed - clock_elapsed;

            sleep(Duration::from_millis(1000 - sync_offset));

            match self.async_command_listen() {
                Command::Quit => return,
                Command::Reset => return self.reset_current_pomodoro(),
                _ => (),
            }

            self.clock.decrement_one_second();
            self.draw_break_screen();

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

    /// Draws the work clock on the screen.
    pub fn draw_work_screen(&mut self) {
        let clock = self.clock.gen_clock("Time to Work!");
        self.draw_work_count();
        self.draw_controls_help();
        self.draw_clock(clock);
    }

    /// Draws the break clock on the screen.
    pub fn draw_break_screen(&mut self) {
        let clock = self.clock.gen_clock("Time to Chill");
        self.draw_work_count();
        self.draw_clock(clock);
        self.draw_controls_help();
    }

    /// Takes in an input string and prints it centered on the screen
    /// Returns the last line it was printed on in case another method
    /// needs to know that to clear space after it.
    pub fn draw_centered(&mut self, item: &str, height_offset: Option<u16>) -> usize {
        let lines = item.lines();
        let line_vec = item.lines().collect::<Vec<_>>();

        let h = line_vec.len() as u16;
        let w = line_vec[1].chars().count();

        let height_offset = if let Some(offset) = height_offset {
            offset
        } else {
            0
        };

        let mut last_i = 0;
        for (i, line) in lines.enumerate() {
            write!(
                self.stdout,
                "{}{}{}",
                cursor::Goto(
                    (&self.width / 2) - (w / 2) as u16,
                    (&self.height / 2) - (h / 2) + height_offset + i as u16
                ),
                clear::CurrentLine,
                line,
            )
            .unwrap();

            last_i = i;
        }

        last_i
    }

    /// Draws the clock on the screen.
    pub fn draw_clock(&mut self, clock: String) {
        self.draw_centered(&clock, None);
    }

    /// Draws the current work count on the screen.
    ///
    /// # Example:
    ///  
    /// "Work Period 1 or 4"
    pub fn draw_work_count(&mut self) {
        write!(
            self.stdout,
            "\r\n{}{}Work Period {} of 4",
            cursor::Goto((&self.width / 2) - 9, (&self.height / 2) + 5),
            clear::CurrentLine,
            &self.pomodoro_tracker.current_order.unwrap(),
        )
        .unwrap();
    }

    /// Draws the section of the screen when the work and break clocks are rolling that actively
    /// remind user of async commands they can issue at any time to restart or quit the current
    /// cycle.
    pub fn draw_controls_help(&mut self) {
        self.draw_centered(CONTROLS, Some(8));
    }

    fn clear_lines(&mut self, lines: &[usize; 2]) {
        for i in lines[0]..lines[1] {
            write!(
                self.stdout,
                "{}{}",
                cursor::Goto(1, (&self.height / 2) - 3 + i as u16),
                clear::CurrentLine,
            )
            .unwrap();
        }
    }

    /// Displays the pomodoro menu that is seen either at the start (large menu) or between
    /// pomodoros (small menu)
    pub fn display_menu(&mut self, menu: Option<&'static str>) {
        let menu = if let Some(menu) = menu {
            menu
        } else {
            POMODORO_MENU
        };

        let last_i = self.draw_centered(menu, None);

        self.clear_lines(&[last_i + 1, last_i + 8]);

        self.stdout.flush().unwrap();

        match self.wait_for_next_command() {
            Command::Start => self.begin_cycle(),
            Command::Quit => return,
            Command::Reset => (),
            Command::None => (),
        }
    }

    /// WAITS (in a loop) for the next user command (happens between pomodoros).
    pub fn wait_for_next_command(&mut self) -> Command {
        let mut command = Command::None;

        while let Command::None = command {
            let mut buf = [0];
            self.stdin.read(&mut buf).unwrap();
            command = match buf[0] {
                b's' => Command::Start,
                b'r' => Command::Reset,
                b'q' => Command::Quit,
                _ => continue,
            }
        }

        command
    }

    /// listens for the next command while clock is counting down in a non-blocking (async)
    /// fashion. 
    pub fn async_command_listen(&mut self) -> Command {
        let mut buf = [0];
        self.stdin.read(&mut buf).unwrap();
        let command = match buf[0] {
            b'r' => Command::Reset,
            b'q' => Command::Quit,
            _ => Command::None,
        };

        command
    }
}

/// A simple state tracker that keeps track of the pomodoro state, the current order we are in (1
/// to 4 or None if we haven't begun our first pomodoro yet), and when the current pomodoro was
/// started at.  (Started at is None between pomodoros).
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

    fn decrement_cycle(&mut self) {
        let new_order = match self.current_order {
            Some(num) if num > 1 => Some(num - 1),
            Some(num) if num == 1 => None,
            _ => Some(1),
        };
        self.current_order = new_order;
    }

    /// Gets the current order we are at within a pomdoro cycle (4 pomodoros per cycle).
    pub fn get_order(&self) -> Option<i32> {
        self.current_order
    }

    /// Sets the current work state by storing the current time, setting our pomodoro state to
    /// PomodoroState::Working, and then incrementing the cycle by one.
    pub fn set_work_state(&mut self) {
        let now = Instant::now();
        self.started_at = Some(now);

        self.current_state = PomodoroState::Working;
        self.increment_cycle();
    }

    /// Sets the break state (ShortBreak, LongBreak, or None).
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

/// Simple struct to translate user keystrokes into command types we can enforce with matches.
pub enum Command {
    Start,
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

/// A simple clock struct that displays minutes and seconds, and has methods for drawing a nice
/// border around the current dispalyed time.
pub struct Clock {
    minutes: u64,
    seconds: u64,
}

impl Clock {
    /// Instantiates a new clock at 00:00.
    pub fn new() -> Clock {
        Clock {
            minutes: 0,
            seconds: 0,
        }
    }

    /// Sets clock time in absolute milliseconds.
    pub fn set_time_ms(&mut self, ms: u64) {
        self.minutes = (ms / (1000 * 60)) % 60;
        self.seconds = (ms / 1000) % 60;
    }

    /// Sets clock time in absolute minutes.
    pub fn set_time_minutes(&mut self, minutes: u64) {
        self.set_time_ms(minutes * 60000);
    }

    /// Decrements the clock by one full second.
    pub fn decrement_one_second(&mut self) {
        let mut time_in_ms = self.get_ms_from_time();
        time_in_ms -= 1000;
        self.set_time_ms(time_in_ms);
    }

    /// Translate the current clock time back into milliseconds
    pub fn get_ms_from_time(&mut self) -> u64 {
        (self.minutes * 60000) + (self.seconds * 1000)
    }

    /// Translates the clock struct into a human readable string.
    ///
    /// # Example
    ///
    /// ```rust
    /// let mut clock = pomodoro::Clock::new();
    ///
    /// clock.set_time_minutes(5);
    ///
    /// assert_eq!(clock.get_time(), "05:00".to_string());
    /// ```
    pub fn get_time(&self) -> String {
        format!("{:02}:{:02}", self.minutes, self.seconds)
    }

    /// Given a message ("Get to Work", or "Time to Chill") this will generate a nicely displayed
    /// clock with the message added.
    pub fn gen_clock(&self, message: &str) -> String {
        let clock = format!("
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

// Initializes the pomodoro session (this is run from start to finish)
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

/// Basic run function that is called from the binary.  Takes the current terminal size, and config
/// from terminal flags and passes that into our init function
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
        assert_eq!(clock.get_time(), "01:00");
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
        assert_eq!(pstate.get_order(), Some(1));
    }
}
