//! Gameplay sound requests: card, ledger, script, UI-click, auction and result
//! firework cues (original 4ad760 owner). The binary's `gameplay_sound::Player`
//! serves them.
use crate::pe::PeImage;
use std::sync::mpsc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operation {
    /// Native 4af140 calls the backend's +24 method before releasing retained
    /// city handles. Auction closure releases its murmur handle afterward.
    Reset,
    Targeted {
        actor_is_current: bool,
    },
    AuctionAmbientCreate,
    AuctionAmbientRelease(u32),
    /// Native card-panel callback 41b430 chooses one of two cues from the
    /// original 20-byte card row's fourth word.
    CardOpen {
        positive: bool,
    },
    /// Original result particle 4549a9/453c1c cue, after a 50 Hz firework tick.
    ResultFirework(FireworkCue),
    /// 465980 dispatches these only when 4ecc20 completes an auction actor
    /// animation, after the producer has stored its participant state.
    AuctionActor(AuctionActorCue),
    Script {
        name: String,
    },
    /// Native 48a5a0 plays a 60-point buy/sell cue for the local ledger.
    Ledger {
        sale: bool,
    },
    /// Original 42f33e/42ca30/4433e8 widget activations.
    UiLightClick,
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

/// Script sound requests can arise while the UI is waiting for a worker reply.
/// Queue them without a reciprocal wait so a status/snapshot request cannot
/// deadlock the simulation and renderer threads.
pub fn enqueue(sender: &Option<mpsc::Sender<Request>>, operation: Operation) -> Result<(), String> {
    let Some(sender) = sender else { return Ok(()) };
    let (reply, _answer) = mpsc::channel();
    let _ = sender.send(Request { operation, reply });
    Ok(())
}

pub fn card_open_positive(image: &PeImage<'_>, card: u32) -> Result<bool, String> {
    let row = crate::event_cards::lifecycle::card_row(image, card)?;
    Ok(i32::from_le_bytes(row[12..16].try_into().unwrap()) <= 6)
}

/// Original 4532c0 branch names and the kind-3 4549a9 launch cue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FireworkCue {
    Launch1,
    Launch2,
    EchoCrackle,
    EchoSingle,
    LaunchWhistle,
}

/// Auction actor animation cues (465980).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuctionActorCue {
    Bid {
        participant: u32,
        character: u32,
        suppressed: bool,
    },
    Withdraw {
        participant: u32,
        character: u32,
        suppressed: bool,
    },
    Win {
        character: u32,
    },
    Applause,
}
