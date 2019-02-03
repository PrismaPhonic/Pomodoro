extern crate pomodoro;
use std::process;

use structopt::StructOpt;

fn main() {
    let config = pomodoro::PomodoroConfig::from_args();

    if let Err(e) = pomodoro::run(config) {
        eprintln!("Application error: {}", e);
        process::exit(1);
    }
}
