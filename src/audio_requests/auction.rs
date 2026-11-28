//! Auction announcement and turn-timer sound requests. The retained worker
//! receives the sound handle synchronously, just as original 4ad760 returns a
//! handle to each controller. The frontend's `auction_audio::Player` serves them.
use std::sync::mpsc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Announcement,
    TurnTimer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    /// Original 4af140 calls the backend's +24 method before separate handle
    /// releases. Stop playback but keep IDs for later explicit Release calls.
    Reset,
    Create(Kind),
    Release(u32),
}

pub struct Request {
    pub operation: Operation,
    pub reply: mpsc::Sender<Result<u32, String>>,
}

pub fn request(
    sender: &Option<mpsc::Sender<Request>>,
    operation: Operation,
) -> Result<u32, String> {
    let sender = sender
        .as_ref()
        .ok_or("auction audio has no live UI owner")?;
    let (reply, answer) = mpsc::channel();
    if sender.send(Request { operation, reply }).is_err() {
        return Ok(0);
    }
    // A retired Inspection may drop the reply while its worker is closing.
    answer.recv().unwrap_or(Ok(0))
}

pub fn enqueue(sender: &Option<mpsc::Sender<Request>>, operation: Operation) -> Result<(), String> {
    let Some(sender) = sender else { return Ok(()) };
    let (reply, _answer) = mpsc::channel();
    let _ = sender.send(Request { operation, reply });
    Ok(())
}
