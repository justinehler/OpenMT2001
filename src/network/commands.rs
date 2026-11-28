//! Bounded remote command qualification (0x450490 -> 0x451720 -> 0x450a30).
//!
//! Native payload schemas below exclude the common 20-byte header (kind at +4,
//! size +8, Total due +12, delayed byte +16). Construction and the short
//! command16 transfer are qualified. The 52-byte command16 auction descriptor is
//! a DIFFERENT schema, not an alternative way to submit a property transfer.
//!
//! | Family | Native payload | Remote support |
//! |---|---|---|
//! | 0f construction | category i8, rectangle i8[4], height i8, variation/business/kind u32, quality/owner i8, components f32[4] (construction::Command::packet) | supported; quote components retained |
//! | 10 demolition | category i8, building u16, amount f32 | unsupported |
//! | 11 business open | kind u32, building u32, owner u32, stock_argument u8 | unsupported |
//! | 12 conversion | kind u32, building u32, owner u32 | unsupported |
//! | 13 business sale | business u32, player u8 | unsupported |
//! | 15 takeover | building u32 | unsupported |
//! | 16 property transfer | amount i32, category i16, new owner u8 | supported |
//! | 16/19/1d/1e/1f/20 auction | 52-byte auction descriptor | unsupported |
//! | 18 terms | business u32, stock i16[6], prices f32[6], rent f32 | unsupported |
//! | 1a event card | card u32, player u32, target i32 | unsupported |
//! | player messages | player_messages::Packet (48 bytes), recipient/sender/kind/args | unsupported |
//! | category-ready/elimination/outcome | internal runtime events | unsupported |
//! | other native IDs / shapes | no qualified decoder | unsupported |
//!
//! Every other kind/schema returns Unsupported. This is not full multiplayer
//! command coverage or auction/offer admission. The submitting gameplay caller
//! still owns its original quote/offer policy; a wire owner is never authority.
use super::{Connection, Error, Message, Owner, Result, SLOTS, SessionBinding, Transfer};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    /// Monotonic per gameplay peer, independent of the transport sequence.
    pub serial: u64,
    /// Original one-based compact gameplay owner, never a lobby row/token.
    pub owner: u8,
    pub kind: u32,
    pub payload: Vec<u8>,
}
impl Request {
    pub fn construction(serial: u64, command: &crate::construction::Command) -> Self {
        Self {
            serial,
            owner: command.owner as u8,
            kind: 0x0f,
            payload: command.packet()[20..].to_vec(),
        }
    }
    pub fn construction_payload(&self) -> Result<crate::construction::Command> {
        if self.kind != 0x0f || self.payload.len() != 40 {
            return Err(Error::Unsupported);
        }
        let mut packet = [0; 60];
        packet[4..8].copy_from_slice(&0x0fu32.to_le_bytes());
        packet[20..].copy_from_slice(&self.payload);
        let command = crate::construction::Command::from_packet(&packet).map_err(|_| Error::Row)?;
        if !(0..52).contains(&command.category)
            || !(1..=6).contains(&self.owner)
            || command.owner as u8 != self.owner
            || !crate::building_pool::can_place(&[0; 49], command.rectangle.map(i32::from))
            || command.height < 0
            || command.kind >= 3
            || command.business >= 45
            || !(0..3).contains(&command.quality)
            || command.components.iter().any(|value| !value.is_finite())
        {
            return Err(Error::Row);
        }
        Ok(command)
    }
    pub(super) fn validate(&self) -> Result<()> {
        match self.kind {
            0x0f => self.construction_payload().map(|_| ()),
            0x16 => self.transfer_payload().map(|_| ()),
            _ => Err(Error::Unsupported),
        }
    }
    pub fn transfer(serial: u64, transfer: Transfer) -> Self {
        let mut payload = transfer.amount.to_le_bytes().to_vec();
        payload.extend_from_slice(&i16::from(transfer.category).to_le_bytes());
        payload.push(transfer.owner);
        Self {
            serial,
            owner: transfer.owner,
            kind: 0x16,
            payload,
        }
    }
    pub fn transfer_payload(&self) -> Result<Transfer> {
        if self.kind != 0x16 || self.payload.len() != 7 {
            return Err(Error::Unsupported);
        }
        let amount =
            i32::from_le_bytes(self.payload[..4].try_into().map_err(|_| Error::Truncated)?);
        let category = i16::from_le_bytes(
            self.payload[4..6]
                .try_into()
                .map_err(|_| Error::Truncated)?,
        );
        if !(0..52).contains(&category)
            || !(1..=6).contains(&self.owner)
            || self.payload[6] != self.owner
        {
            return Err(Error::Row);
        }
        Ok(Transfer {
            category: category as u8,
            owner: self.owner,
            amount,
            due: 0,
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commit {
    pub order: u64,
    pub due: u32,
    pub request: Request,
}
/// Minted by the worker after Owner::receive admits the ordered frame. It can
/// only be consumed against the same completed world and adopted connection.
#[derive(Debug)]
pub struct Ingress {
    pub(super) binding: SessionBinding,
    pub(super) connection: Connection,
    pub(super) message: Message,
}

/// Simulation-thread command adapter. All simulation storage is borrowed from
/// the actual Session; this object owns neither a shadow queue nor a clock.
pub struct Adapter {
    binding: SessionBinding,
    host: bool,
    local_peer: u8,
    owners: Vec<Option<u8>>,
    next_request: u64,
    requests: [Option<u64>; SLOTS],
    next_commit: u64,
    next_enqueued: u64,
    deadline: u32,
    closed_tick: Option<u32>,
    pending_local: std::collections::VecDeque<Request>,
}
impl Adapter {
    pub fn new(
        binding: SessionBinding,
        host: bool,
        local_peer: u8,
        owners: Vec<Option<u8>>,
    ) -> Result<Self> {
        if owners.is_empty()
            || owners.len() > SLOTS
            || owners.first() != Some(&Some(0))
            || local_peer as usize >= SLOTS
            || host != (local_peer == 0)
        {
            return Err(Error::Row);
        }
        let mut seen = [false; SLOTS];
        for peer in owners.iter().flatten().copied() {
            let entry = seen.get_mut(peer as usize).ok_or(Error::Peer)?;
            if *entry {
                return Err(Error::Peer);
            }
            *entry = true;
        }
        if !seen[local_peer as usize] {
            return Err(Error::Peer);
        }
        Ok(Self {
            binding,
            host,
            local_peer,
            owners,
            next_request: 0,
            requests: [None; SLOTS],
            next_commit: 0,
            next_enqueued: 0,
            deadline: 25,
            closed_tick: Some(0),
            pending_local: Default::default(),
        })
    }
    pub fn deadline(&self) -> u32 {
        self.deadline
    }
    pub fn is_host(&self) -> bool {
        self.host
    }
    /// Original periodic dispatch skips kind1 AI when networked. Only the
    /// authority runs those controllers; outgoing commands still use the real
    /// network routing flags, independently of this controller-only gate.
    pub fn configure_players(&self, controls: &mut crate::player_tick::Controls) {
        controls.network_enabled = true;
        controls.network_override = self.host;
    }
    /// 47f5cf..47f616 gates only the AI event-receiver ticks on authority.
    /// The subsequent 470b80 bookkeeping/evaluation pass runs on every peer.
    pub fn configure_hourly(&self, controls: &mut crate::hourly::Controls) {
        controls.network_enabled = true;
        controls.network_override = self.host;
    }
    fn authorize(&self, peer: u8, request: &Request) -> Result<()> {
        let player = usize::from(request.owner)
            .checked_sub(1)
            .ok_or(Error::Row)?;
        // AI rows have no connection. Only the authority may act for them;
        // remote peers can act only for their admitted compact human row.
        if !matches!(self.owners.get(player), Some(Some(p)) if *p == peer)
            && !(peer == 0 && self.owners.get(player) == Some(&None))
        {
            return Err(Error::Peer);
        }
        if self.requests[peer as usize].is_some_and(|last| request.serial <= last) {
            return Err(Error::Sequence);
        }
        request.validate()?;
        Ok(())
    }
    /// Client only sends a request. Authority assigns Total+25 exactly once,
    /// then its local queue consumes the very same commit as every client.
    pub fn request(&mut self, owner: &Owner, request: Request, tick: u32) -> Result<Message> {
        owner.validate_binding(self.binding)?;
        self.authorize(self.local_peer, &request)?;
        if self.host && self.closed_tick == Some(tick) {
            return Err(Error::Phase);
        }
        if self.host {
            self.commit(self.local_peer, request, tick)
        } else {
            Ok(Message::CommandRequest(request))
        }
    }
    pub fn next_transfer(&mut self, transfer: Transfer) -> Result<Request> {
        let serial = self.next_request;
        self.next_request = serial.checked_add(1).ok_or(Error::Exhausted)?;
        Ok(Request::transfer(serial, transfer))
    }
    pub fn next_construction(&mut self, command: &crate::construction::Command) -> Result<Request> {
        let serial = self.next_request;
        self.next_request = serial.checked_add(1).ok_or(Error::Exhausted)?;
        Ok(Request::construction(serial, command))
    }
    fn commit(&mut self, peer: u8, request: Request, tick: u32) -> Result<Message> {
        self.authorize(peer, &request)?;
        // 451720 adds authority Total+25; clients never call that producer.
        // Reject the unresolved native -1/wrapped deadline instead of applying early.
        let due = tick
            .checked_add(25)
            .filter(|due| *due != u32::MAX)
            .ok_or(Error::Exhausted)?;
        let order = self.next_commit;
        self.next_commit = order.checked_add(1).ok_or(Error::Exhausted)?;
        self.requests[peer as usize] = Some(request.serial);
        Ok(Message::CommandCommit(Commit {
            order,
            due,
            request,
        }))
    }
    pub fn rejection(&self, ingress: &Ingress) -> Option<(Connection, u64)> {
        match &ingress.message {
            Message::CommandRequest(request) => Some((ingress.connection, request.serial)),
            _ => None,
        }
    }
    pub fn receive(&mut self, owner: &Owner, ingress: Ingress, tick: u32) -> Result<Message> {
        if ingress.binding != self.binding {
            return Err(Error::World);
        }
        owner.validate_connection(self.binding, ingress.connection, !self.host)?;
        match ingress.message {
            Message::CommandRequest(request) if self.host => {
                self.commit(ingress.connection.slot(), request, tick)
            }
            message @ (Message::CommandCommit(_)
            | Message::CommandDeadline { .. }
            | Message::CommandRejected { .. })
                if !self.host =>
            {
                Ok(message)
            }
            _ => Err(Error::Tag),
        }
    }
    /// Kind8 shares commit ordering: it may only be published after the host
    /// has emitted every preceding commit to each ordered transport stream.
    pub fn advance_deadline(&self, tick: u32) -> Result<Message> {
        if !self.host {
            return Err(Error::Peer);
        }
        Ok(Message::CommandDeadline {
            after: self.next_commit,
            tick: tick.checked_add(25).ok_or(Error::Exhausted)?,
        })
    }
    pub fn admit_deadline(&mut self, after: u64, tick: u32) -> Result<()> {
        if after != self.next_enqueued || tick <= self.deadline {
            return Err(Error::Sequence);
        }
        self.deadline = tick;
        Ok(())
    }
    #[cfg(feature = "lua4")]
    fn apply_deadline(
        &mut self,
        after: u64,
        deadline: u32,
        tick: u32,
        pause: &mut crate::pause::State,
        tasks: &mut [[u8; 20]],
    ) -> std::result::Result<(), String> {
        self.admit_deadline(after, deadline)
            .map_err(|e| e.to_string())?;
        if deadline > tick {
            pause.unpause(1, |change, _| {
                change.apply(tasks);
                Ok(())
            })?;
        }
        Ok(())
    }
    #[cfg(feature = "lua4")]
    pub fn submit(
        &mut self,
        worker: &super::lobby::worker::Handle,
        command: crate::live_commands::Command,
        queue: &mut crate::live_commands::Queue,
        categories: &mut [[u8; 0x17c]; 52],
        tick: u32,
    ) -> std::result::Result<(), String> {
        let request = match command {
            crate::live_commands::Command::Build(command) => self.next_construction(&command),
            crate::live_commands::Command::PropertyTransfer {
                category,
                owner,
                amount,
            } => {
                let category =
                    u8::try_from(category).map_err(|_| "Remote category outside byte range")?;
                self.next_transfer(Transfer {
                    category,
                    owner,
                    amount,
                    due: 0,
                })
            }
            _ => return Err(Error::Unsupported.to_string()),
        };
        let request = request.map_err(|e| e.to_string())?;
        if self.host && self.closed_tick == Some(tick) {
            worker.validate_binding(self.binding)?;
            self.authorize(self.local_peer, &request)
                .map_err(|e| e.to_string())?;
            if self.pending_local.len() >= 64 {
                return Err("Authority command queue is full".into());
            }
            self.pending_local.push_back(request);
            return Ok(());
        }
        self.authorize(self.local_peer, &request)
            .map_err(|e| e.to_string())?;
        check_placement(&request, categories).map_err(|e| e.to_string())?;
        let message = self
            .request(worker.gameplay_owner(), request, tick)
            .map_err(|e| e.to_string())?;
        if let crate::network::Message::CommandCommit(commit) = &message {
            self.enqueue(commit, queue, categories, tick)
                .map_err(|e| e.to_string())?;
        }
        worker.send_gameplay(self.binding, None, message)
    }
    /// Ordered ingress, kind8 resume and five-Hz authority deadlines. Borrow the
    /// actual Session queue/pause/tasks; no replacement timer records are made.
    #[cfg(feature = "lua4")]
    pub fn pump(
        &mut self,
        worker: &super::lobby::worker::Handle,
        tick: u32,
        queue: &mut crate::live_commands::Queue,
        categories: &mut [[u8; 0x17c]; 52],
        pause: &mut crate::pause::State,
        tasks: &mut [[u8; 20]],
    ) -> std::result::Result<(), String> {
        worker.validate_binding(self.binding)?;
        while let Some(update) = worker.poll() {
            if let crate::network::lobby::worker::Update::Failed(error) = update {
                return Err(error);
            }
        }
        // The previous kind8 already authorized clients through tick+25.
        // Requests arriving in another frame at that same Total must wait for
        // the next authority tick, whose original +25 due is strictly later.
        if self.host && self.closed_tick == Some(tick) {
            return Ok(());
        }
        while let Some(request) = self.pending_local.pop_front() {
            check_placement(&request, categories).map_err(|e| e.to_string())?;
            let message = self
                .request(worker.gameplay_owner(), request, tick)
                .map_err(|e| e.to_string())?;
            if let Message::CommandCommit(commit) = message {
                self.enqueue(&commit, queue, categories, tick)
                    .map_err(|e| e.to_string())?;
                worker.send_gameplay(self.binding, None, Message::CommandCommit(commit))?;
            }
        }
        while let Some(ingress) = worker.poll_gameplay() {
            let rejection = self.rejection(&ingress);
            let placement = match &ingress.message {
                Message::CommandRequest(request) if self.host => {
                    check_placement(request, categories)
                }
                _ => Ok(()),
            };
            let message = match placement
                .and_then(|()| self.receive(worker.gameplay_owner(), ingress, tick))
            {
                Ok(message) => message,
                Err(error) if self.is_host() => {
                    if let Some((target, serial)) = rejection {
                        worker.send_gameplay(
                            self.binding,
                            Some(target),
                            Message::CommandRejected {
                                serial,
                                unsupported: error == Error::Unsupported,
                            },
                        )?;
                    }
                    continue;
                }
                Err(error) => return Err(error.to_string()),
            };
            match message {
                Message::CommandCommit(commit) => {
                    self.enqueue(&commit, queue, categories, tick)
                        .map_err(|e| e.to_string())?;
                    if self.is_host() {
                        worker.send_gameplay(self.binding, None, Message::CommandCommit(commit))?;
                    }
                }
                Message::CommandDeadline {
                    after,
                    tick: deadline,
                } => {
                    self.apply_deadline(after, deadline, tick, pause, tasks)?;
                }
                Message::CommandRejected {
                    serial,
                    unsupported,
                } => {
                    return Err(format!(
                        "Remote command {serial} rejected: {}",
                        if unsupported {
                            "unsupported family/schema"
                        } else {
                            "owner or sequence"
                        }
                    ));
                }
                _ => return Err(Error::Unsupported.to_string()),
            }
        }
        if self.is_host()
            && tick.is_multiple_of(5)
            && tick
                .checked_add(25)
                .is_some_and(|next| next > self.deadline())
        {
            let message = self.advance_deadline(tick).map_err(|e| e.to_string())?;
            if let Message::CommandDeadline {
                after,
                tick: deadline,
            } = message
            {
                // Queue the deadline AFTER all commits; never unblock locally
                // if the bounded outbound channel could not accept it.
                worker.send_gameplay(
                    self.binding,
                    None,
                    Message::CommandDeadline {
                        after,
                        tick: deadline,
                    },
                )?;
                self.admit_deadline(after, deadline)
                    .map_err(|e| e.to_string())?;
                self.closed_tick = Some(tick);
            }
        }
        Ok(())
    }
    #[cfg(feature = "lua4")]
    pub fn enqueue(
        &mut self,
        commit: &Commit,
        queue: &mut crate::live_commands::Queue,
        categories: &mut [[u8; 0x17c]; 52],
        tick: u32,
    ) -> Result<()> {
        // Authority incremented its order when minting this commit. Clients
        // advance only after admitting it into the actual live queue.
        let expected = self.next_enqueued;
        if commit.order != expected || commit.due <= tick || commit.due == u32::MAX {
            return Err(Error::Sequence);
        }
        let request = &commit.request;
        request.validate()?;
        if self
            .owners
            .get(usize::from(request.owner).wrapping_sub(1))
            .is_none()
        {
            return Err(Error::Row);
        }
        let next = self.next_enqueued.checked_add(1).ok_or(Error::Exhausted)?;
        let command = match request.kind {
            0x0f => {
                check_placement(request, categories)?;
                let command = request.construction_payload()?;
                // Original Build0f reserves occupancy before the delayed consumer.
                // Reserve on every peer only after authority admission, never on
                // a speculative client request. The quote is never recomputed.
                let occupancy = (&mut categories[command.category as usize][0x9c..0xcd])
                    .try_into()
                    .map_err(|_| Error::Row)?;
                crate::building_pool::fill_occupancy(
                    occupancy,
                    command.rectangle.map(i32::from),
                    command.height as u8,
                )
                .map_err(|_| Error::Row)?;
                crate::live_commands::Command::Build(command)
            }
            0x16 => {
                let t = request.transfer_payload()?;
                crate::live_commands::Command::PropertyTransfer {
                    category: i16::from(t.category),
                    owner: t.owner,
                    amount: t.amount,
                }
            }
            _ => return Err(Error::Unsupported),
        };
        queue.restore_queued(
            command,
            crate::scheduler::ClockDomain::Total,
            commit.due,
            commit.due,
            false,
        );
        self.next_enqueued = next;
        if !self.host {
            self.next_commit = self.next_enqueued;
        }
        Ok(())
    }
}

#[cfg(feature = "lua4")]
fn check_placement(request: &Request, categories: &[[u8; 0x17c]; 52]) -> Result<()> {
    if request.kind == 0x0f {
        let command = request.construction_payload()?;
        let occupancy = categories[command.category as usize][0x9c..0xcd]
            .try_into()
            .map_err(|_| Error::Row)?;
        if !crate::building_pool::can_place(occupancy, command.rectangle.map(i32::from)) {
            return Err(Error::Row);
        }
    }
    Ok(())
}

/// Original 4bac90 reason1, idempotent at a network limit. The retained group2
/// timer bytes are changed in place, not replaced or registered again.
pub fn pause_at_limit(
    pause: &mut crate::pause::State,
    tasks: &mut [[u8; 20]],
) -> std::result::Result<(), String> {
    if pause.bytes[1] == 0 {
        pause.pause(1, |change, _| {
            change.apply(tasks);
            Ok(())
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
