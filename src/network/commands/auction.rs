//! Auction command schemas and host serialization (0x451720/0x4644e0).
use super::*;
#[cfg(feature = "lua4")]
use crate::player_manager::put;
use crate::player_manager::word;

impl Request {
    /// Keep the native delayed flag and announcement delay separate from the
    /// authority's Total. Only the authority converts them to an absolute due.
    pub fn auction(serial: u64, owner: u8, descriptor: [u8; 52], additional: u32) -> Self {
        let mut payload = descriptor.to_vec();
        payload.extend_from_slice(&additional.to_le_bytes());
        Self {
            serial,
            owner,
            kind: word(&descriptor, 4),
            payload,
        }
    }
    pub(super) fn is_auction(&self) -> bool {
        matches!(self.kind, 0x16 | 0x19 | 0x1d..=0x20 | 0x27..=0x2a) && self.payload.len() == 56
    }
    pub fn auction_payload(&self) -> Result<([u8; 52], u32)> {
        if !self.is_auction() {
            return Err(Error::Unsupported);
        }
        let d: [u8; 52] = self.payload[..52]
            .try_into()
            .map_err(|_| Error::Truncated)?;
        let additional = word(&self.payload, 52);
        if word(&d, 4) != self.kind || d[16] > 1 || self.owner > 6 {
            return Err(Error::Row);
        }
        match self.kind {
            0x16 if i16::from_le_bytes([d[24], d[25]]) < 0
                || i16::from_le_bytes([d[24], d[25]]) >= 52
                || d[26] > 6 =>
            {
                return Err(Error::Row);
            }
            0x19 if word(&d, 20) >= 52 => return Err(Error::Row),
            0x1d if word(&d, 20) >= 52 || (word(&d, 24) != u32::MAX && word(&d, 24) >= 6) => {
                return Err(Error::Row);
            }
            0x1f if word(&d, 20) > 4
                || (matches!(word(&d, 20), 0..=2) && word(&d, 24) >= 6)
                || (word(&d, 20) == 4 && word(&d, 24) > 5) =>
            {
                return Err(Error::Row);
            }
            0x27..=0x2a if d[0x15] >= 6 || d[16] != 0 || additional != 0 => return Err(Error::Row),
            _ => (),
        }
        if self.kind == 0x1f && (d[16] != 0 || additional != 0) {
            return Err(Error::Row);
        }
        if matches!(self.kind, 0x1e | 0x20) && d[16] != 1 {
            return Err(Error::Row);
        }
        Ok((d, additional))
    }
    /// Only the authority emits these. Participant bid/pass/leave actions and
    /// a human owner's offer acceptance (kind 29 for that owner's row) are the
    /// exceptions.
    pub(super) fn auction_authority(&self) -> bool {
        self.is_auction()
            && (self.kind != 0x1f || self.payload.get(20).is_some_and(|action| *action >= 3))
            && !self.is_offer_acceptance()
    }
    /// 0x48b350 choice 0: the owner who accepts an offer sends the kind-29
    /// admission. Its bidder word is the proposer, so the request names the
    /// accepting owner's row instead.
    fn is_offer_acceptance(&self) -> bool {
        self.kind == 0x1d && self.owner != 0
    }
    pub(super) fn command_due(&self, tick: u32) -> Result<u32> {
        if self.is_auction() {
            let (d, additional) = self.auction_payload()?;
            if d[16] == 0 {
                return Ok(0);
            }
            return tick
                .checked_add(25)
                .and_then(|tick| tick.checked_add(additional))
                .filter(|due| *due != u32::MAX)
                .ok_or(Error::Exhausted);
        }
        tick.checked_add(25)
            .filter(|due| *due != u32::MAX)
            .ok_or(Error::Exhausted)
    }
}
impl Adapter {
    pub(super) fn authorize_auction(&self, peer: u8, request: &Request) -> Result<()> {
        let (d, _) = request.auction_payload()?;
        if self.requests[usize::from(peer)].is_some_and(|last| request.serial <= last) {
            return Err(Error::Sequence);
        }
        if request.auction_authority() {
            return if peer == 0 && request.owner == 0 {
                Ok(())
            } else {
                Err(Error::Peer)
            };
        }
        let player = if request.is_offer_acceptance() {
            usize::from(request.owner) - 1
        } else {
            word(&d, 24) as usize
        };
        if request.owner as usize != player + 1 {
            return Err(Error::Peer);
        }
        if matches!(self.owners.get(player), Some(Some(p)) if *p == peer)
            || (peer == 0 && self.owners.get(player) == Some(&None))
        {
            Ok(())
        } else {
            Err(Error::Peer)
        }
    }
    /// A phase change deferred behind a published kind-8 window must apply
    /// before another progress pass can emit that same transition again.
    pub fn auction_control_pending(&self) -> bool {
        self.pending_local
            .iter()
            .any(|(request, _)| request.kind == 0x1f && request.auction_authority())
    }
    #[cfg(feature = "lua4")]
    #[allow(clippy::too_many_arguments)]
    pub fn submit_auction(
        &mut self,
        worker: &super::super::lobby::worker::Handle,
        descriptor: [u8; 52],
        additional: u32,
        queue: &mut crate::live_commands::Queue,
        categories: &mut [[u8; 380]; 52],
        tick: u32,
    ) -> std::result::Result<(), String> {
        let owner = if word(&descriptor, 4) == 0x1f && word(&descriptor, 20) <= 2 {
            u8::try_from(word(&descriptor, 24).wrapping_add(1))
                .map_err(|_| Error::Peer.to_string())?
        } else {
            0
        };
        let serial = self.next_serial().map_err(|e| e.to_string())?;
        let request = Request::auction(serial, owner, descriptor, additional);
        self.submit_request(
            worker,
            request,
            false,
            queue,
            categories,
            Targets::default(),
            tick,
        )
    }
    /// A human owner accepting an offer (0x48b350 choice 0): the scheduled
    /// 52-byte kind-29 admission request. A client sends it to the authority,
    /// whose router (0x451720) schedules it at Total+25 for every peer, where
    /// the ordinary auction admission (0x465330) applies it. `acceptor` is
    /// the accepting owner's zero-based row.
    #[cfg(feature = "lua4")]
    pub fn submit_offer_acceptance(
        &mut self,
        worker: &super::super::lobby::worker::Handle,
        descriptor: [u8; 52],
        acceptor: u8,
        queue: &mut crate::live_commands::Queue,
        categories: &mut [[u8; 380]; 52],
        tick: u32,
    ) -> std::result::Result<(), String> {
        if word(&descriptor, 4) != 0x1d || descriptor[16] != 1 {
            return Err(Error::Row.to_string());
        }
        let owner = acceptor
            .checked_add(1)
            .filter(|owner| usize::from(*owner) <= SLOTS)
            .ok_or_else(|| Error::Row.to_string())?;
        let serial = self.next_serial().map_err(|e| e.to_string())?;
        let request = Request::auction(serial, owner, descriptor, 0);
        self.submit_request(
            worker,
            request,
            false,
            queue,
            categories,
            Targets::default(),
            tick,
        )
    }
    #[cfg(feature = "lua4")]
    pub(super) fn enqueue_auction(
        &self,
        request: &Request,
        due: u32,
    ) -> Result<crate::live_commands::Command> {
        let (mut descriptor, _) = request.auction_payload()?;
        put(&mut descriptor, 12, due);
        Ok(crate::live_commands::Command::Auction { descriptor })
    }
}
