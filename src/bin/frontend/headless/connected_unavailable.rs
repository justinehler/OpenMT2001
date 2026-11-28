//! `--host`/`--join` and `--compare` in the native-free `headless` package.
//! Connected play builds on the interface layer's connected start, so these
//! report that `game headless` is needed instead.
use super::{End, Options, Start};
use std::io::Write;

const UNAVAILABLE: &str =
    "connected play (--host/--join/--compare) needs `game headless` from the game binary";

pub struct Inputs;
pub struct Staged;
pub struct Request;
pub struct Peer;

pub fn inputs(
    _paths: &super::Paths,
    _process: crate::live_host::process_runtime::Handle,
) -> Result<Inputs, String> {
    Err(UNAVAILABLE.into())
}

pub fn stage(_inputs: &Inputs, _request: Request) -> Result<Staged, String> {
    Err(UNAVAILABLE.into())
}

pub fn connect<'a>(
    _inputs: &'a Inputs,
    _staged: Staged,
    _dispatch: Option<crate::process_dispatch::Seed>,
) -> Result<crate::fresh_city::Session<'a>, String> {
    Err(UNAVAILABLE.into())
}

pub fn lobby(
    _start: &Start,
    _image: &openmt2001::pe::PeImage<'_>,
    _paths: &super::Paths,
    _names: &[Vec<u8>; 10],
) -> Result<(Request, Peer), String> {
    Err(UNAVAILABLE.into())
}

pub fn advance(
    _host: &mut crate::live_host::Host,
    _session: &mut crate::fresh_city::Session<'_>,
    _assets: &crate::fresh_city::CommandAssets<'_, '_>,
    _options: &Options,
    _peer: &Peer,
    _out: &mut dyn Write,
) -> Result<End, String> {
    Err(UNAVAILABLE.into())
}

pub fn compare_main(_paths: &[String]) -> i32 {
    eprintln!("{UNAVAILABLE}");
    2
}
