//! `headless`: the game's headless runner without a window, GPU or audio
//! stack (#99). It mounts the same sources as `game headless` and runs the
//! same `live_host::Host::frame` path, so the two print identical output.
//!
//! `headless ARGS` and `headless headless ARGS` both work, so it drops in for
//! `game headless ARGS`.
// The runner's tests need the interface layer and run in openmt-game.
// `cargo test --bins` and `--all-targets` still build this binary as a test,
// so that build is empty.
#![cfg(not(test))]
// The game drives the rest of the Host and session API; only the runner's
// path is live here.
#![allow(dead_code, unused_imports)]
use openmt_headless::{fresh_city, live_host, process_dispatch, session_save};
#[allow(unused_imports)]
use openmt2001::{campaign_store, compatibility, human_dialog, save_menu};
#[path = "../../../src/bin/frontend/headless.rs"]
mod headless;

fn main() {
    let mut args = std::env::args().skip(1).peekable();
    if args.peek().map(String::as_str) == Some("headless") {
        args.next();
    }
    std::process::exit(headless::main(args));
}
