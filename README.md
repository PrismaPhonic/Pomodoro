[![Build Status](https://travis-ci.org/PrismaPhonic/Pomodoro.svg?branch=master)](https://travis-ci.org/PrismaPhonic/Pomodoro)
[![crates.io](http://meritbadge.herokuapp.com/pomodoro)](https://crates.io/crates/pomodoro)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)
[![Released API
docs](https://docs.rs/pomodoro/badge.svg)](https://docs.rs/pomodoro)

# pomodoro

This crate offers you a functional terminal based pomodoro clock.

# Dependencies

 This application works on Linux and OSX, but not Windows (yet). On linux make sure that you
 have libdbus-1 installed - this is an essentialy dependency so that pomodoro can integrate
 with the linux notification system.

# Installation

This clock requires being built with nightly because of an experimental feature I used to keep
the clock in sync and never fluctuating by more than 1ms.  You can install the application with this command:

```terminal
$ cargo +nightly install pomodoro
```

Note: On OSX you don't need to install anything extra. Just use the above terminal command to
install the binary crate with nightly

## Using pomodoro

To use, simply run it. By default it will give you a work time of 25 minutes, short break of 5
minutes and a long break of 20 minutes.

```terminal
$ pomodoro
```

You can pass it terminal flags to customize the times.  `-w` flag will set the work time, `-s`
will set the short break time, and `-l` will set the long break time.  Here's an example that
sets up a custom pomodoro with 30 minute work time, 10 minute short break and 25 minute long
break:

```terminal
$ pomodoro -w 30 -s 10 -l 25
```

All of the controls for starting, quitting or resetting a pomodoro are displayed by the
pomodoro menu on launch. `s` will start your next pomodoro. `q` will take you back to the
menu if you are in a pomodoro, or quit if you are at the menu. `r` will reset the current
pomodoro (back to the head of the work cycle and immediately begin countdown).

Commands are listened for in an asynchronous and non-blocking fashion.

Enjoy!

