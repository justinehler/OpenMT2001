//! Era music and before-six-o'clock sound requests (4952f0, 4ad6f0). The
//! binary's `era_audio::Player` serves them.
use std::sync::mpsc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Select {
        era: u32,
        repeat: bool,
    },
    /// Original result constructor 4b1881 calls 4af140 before result music.
    Stop,
    BeforeSixCreate,
    BeforeSixRelease(u32),
}

pub struct Request {
    pub operation: Operation,
    pub reply: mpsc::Sender<Result<u32, String>>,
}

pub fn request(
    sender: &Option<mpsc::Sender<Request>>,
    operation: Operation,
) -> Result<u32, String> {
    let Some(sender) = sender else { return Ok(0) };
    let (reply, answer) = mpsc::channel();
    if sender.send(Request { operation, reply }).is_err() {
        return Ok(0);
    }
    answer.recv().unwrap_or(Ok(0))
}
