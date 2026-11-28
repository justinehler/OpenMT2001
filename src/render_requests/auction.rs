//! Synchronous original auction scene boundaries between simulation and renderer.
//! A disconnected renderer is an error, never a fabricated prior UI mode. The
//! binary's `auction_bridge::View` serves these requests.
use std::sync::mpsc;

pub enum Operation {
    Prepare {
        characters: Vec<u32>,
        actor_events: mpsc::Receiver<ActorEvent>,
    },
    Activate,
    Restore,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActorEvent {
    Effect { participant: u32, kind: u32 },
    WithdrawNotice(u32),
    BidVoiceGate(bool),
    Advance,
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
        .ok_or("Auction requires an attached live renderer")?;
    let (reply, result) = mpsc::channel();
    sender
        .send(Request { operation, reply })
        .map_err(|_| "Auction renderer disconnected")?;
    result
        .recv()
        .map_err(|_| "Auction renderer dropped its scene response")?
}
