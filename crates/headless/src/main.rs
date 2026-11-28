//! `headless`: the game's headless runner without a window, GPU or audio
//! stack (#99). It mounts the same sources as `game headless` and runs the
//! same `live_host::Host::frame` path, so the two print identical output.
//!
//! `headless ARGS` and `headless headless ARGS` both work, so it drops in for
//! `game headless ARGS`.
// The mounted sources' tests need the interface layer; they run in
// openmt-game. `cargo test --bins` and `--all-targets` still build this
// binary as a test, so that build is empty.
#![cfg(not(test))]
// The game drives the rest of the Host and session API; only the runner's
// path is live here.
#![allow(dead_code, unused_imports)]
use openmt2001::{campaign_store, compatibility, human_dialog, save_menu};
#[path = "../../../src/bin/frontend/fresh_city.rs"]
pub mod fresh_city;
#[path = "../../../src/bin/frontend/headless.rs"]
mod headless;
#[path = "../../../src/bin/frontend/live_host.rs"]
mod live_host;
#[path = "../../../src/bin/frontend/process_dispatch.rs"]
mod process_dispatch;
#[path = "../../../src/bin/frontend/session_save.rs"]
pub mod session_save;

fn main() {
    let mut args = std::env::args().skip(1).peekable();
    if args.peek().map(String::as_str) == Some("headless") {
        args.next();
    }
    std::process::exit(headless::main(args));
}
