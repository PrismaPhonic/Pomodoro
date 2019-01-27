use pomodoro;
use std::process;

fn main() {
    if let Err(e) = pomodoro::run() {
        eprintln!("Application error: {}", e);
        process::exit(1);
    }
}
