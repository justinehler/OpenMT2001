//! Bounded remote command qualification (0x450490 -> 0x451720 -> 0x450a30).
//!
//! Native payload schemas below exclude the common 20-byte header (kind at +4,
//! size +8, Total due +12, delayed byte +16). Construction, business lifecycle/settings and the short
//! command16 transfer are qualified. Native auction descriptors use a separate
//! envelope retaining their delayed flag and additional announcement delay.
//!
//! | Family | Native payload | Remote support |
//! |---|---|---|
//! | 0f construction | category i8, rectangle i8[4], height i8, variation/business/kind u32, quality/owner i8, components f32[4] (construction::Command::packet) | supported; quote components retained |
//! | 10 demolition | +0 category i8, +1 padding, +2 building u16, +4 amount f32 (8 bytes) | supported; linked business owner authorizes; quote retained |
//! | 11 business open | +0 kind u32, +4 building u32, +8 owner u32, +12 stock_argument u8, +13 padding[3] (16 bytes) | supported; building owner authorizes; authority generates completions |
//! | 12 conversion | +0 kind u32, +4 building u32, +8 owner u32 (12 bytes) | supported; linked business owner authorizes |
//! | 13 business sale | +0 business u32, +4 zero-based player u8, +5 padding[3] (8 bytes) | supported; business owner authorizes |
//! | 15 takeover | +0 building u32 (4 bytes) | supported; category landlord authorizes as buyer |
//! | 16 property transfer | amount i32, category i16, new owner u8 | supported |
//! | 16/19/1d/1e/1f/20 auction | native descriptor plus announcement delay | supported; host control, participant-owned actions |
//! | 27..2a auction notifications | addressed 48-byte inbox descriptor | supported; authority only |
//! | 18 terms | +0 business u32, +4 stock i16[6], +16 prices f32[6], +40 rent f32 (44 bytes) | supported; business owner authorizes |
//! | 1a event card | card u32, player u32, target i32 | unsupported |
//! | player messages | player_messages::Packet (48 bytes), recipient/sender/kind/args | unsupported |
//! | category-ready/elimination/outcome | internal runtime events | unsupported |
//! | other native IDs / shapes | no qualified decoder | unsupported |
//!
//! Every other kind/schema returns Unsupported. This is not full multiplayer
//! command coverage. Property offer negotiation remains unqualified. The submitting gameplay caller
//! still owns its original quote/offer policy; a wire owner is never authority.
mod auction;

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
    pub fn demolition(serial: u64, owner: u8, command: &crate::demolition::Command) -> Self {
        Self {
            serial,
            owner,
            kind: 0x10,
            payload: command.packet()[20..].to_vec(),
        }
    }
    pub fn demolition_payload(&self) -> Result<crate::demolition::Command> {
        if self.kind != 0x10 || self.payload.len() != 8 {
            return Err(Error::Unsupported);
        }
        let mut packet = [0; 28];
        packet[4..8].copy_from_slice(&0x10u32.to_le_bytes());
        packet[20..].copy_from_slice(&self.payload);
        let command = crate::demolition::Command::from_packet(&packet).map_err(|_| Error::Row)?;
        if !(0..52).contains(&command.category)
            || !(1..=6).contains(&self.owner)
            || !command.amount.is_finite()
        {
            return Err(Error::Row);
        }
        Ok(command)
    }
    pub fn business_open(serial: u64, command: &crate::business_open::Command) -> Self {
        let mut packet = [0; 36];
        command.write_packet(&mut packet);
        Self {
            serial,
            owner: command.owner as u8,
            kind: 0x11,
            payload: packet[20..].to_vec(),
        }
    }
    pub fn business_open_payload(&self) -> Result<crate::business_open::Command> {
        if self.kind != 0x11 || self.payload.len() != 16 {
            return Err(Error::Unsupported);
        }
        let mut packet = [0; 36];
        packet[4..8].copy_from_slice(&0x11u32.to_le_bytes());
        packet[20..].copy_from_slice(&self.payload);
        let command =
            crate::business_open::Command::from_packet(&packet).map_err(|_| Error::Row)?;
        if command.kind >= 45
            || !(1..=6).contains(&command.owner)
            || command.owner != u32::from(self.owner)
        {
            return Err(Error::Row);
        }
        Ok(command)
    }
    pub fn conversion(serial: u64, command: &crate::business_conversion::Command) -> Self {
        let mut packet = [0; 32];
        command.write_packet(&mut packet);
        Self {
            serial,
            owner: command.owner as u8,
            kind: 0x12,
            payload: packet[20..].to_vec(),
        }
    }
    pub fn conversion_payload(&self) -> Result<crate::business_conversion::Command> {
        if self.kind != 0x12 || self.payload.len() != 12 {
            return Err(Error::Unsupported);
        }
        let command = crate::business_conversion::Command {
            kind: crate::player_manager::word(&self.payload, 0),
            building: crate::player_manager::word(&self.payload, 4),
            owner: crate::player_manager::word(&self.payload, 8),
        };
        if command.kind >= 45
            || !(1..=6).contains(&self.owner)
            || command.owner != u32::from(self.owner)
        {
            return Err(Error::Row);
        }
        Ok(command)
    }
    pub fn business_sale(serial: u64, command: &crate::business_sale::Command) -> Self {
        let mut packet = [0; 28];
        command.write_packet(&mut packet);
        Self {
            serial,
            owner: command.player.wrapping_add(1),
            kind: 0x13,
            payload: packet[20..].to_vec(),
        }
    }
    pub fn business_sale_payload(&self) -> Result<crate::business_sale::Command> {
        if self.kind != 0x13 || self.payload.len() != 8 {
            return Err(Error::Unsupported);
        }
        let command = crate::business_sale::Command {
            business: crate::player_manager::word(&self.payload, 0),
            player: self.payload[4],
        };
        if !(1..=6).contains(&self.owner) || command.player != self.owner - 1 {
            return Err(Error::Row);
        }
        Ok(command)
    }
    pub fn takeover(serial: u64, owner: u8, command: &crate::business_takeover::Command) -> Self {
        let mut packet = [0; 24];
        command.write_packet(&mut packet);
        Self {
            serial,
            owner,
            kind: 0x15,
            payload: packet[20..].to_vec(),
        }
    }
    pub fn takeover_payload(&self) -> Result<crate::business_takeover::Command> {
        if self.kind != 0x15 || self.payload.len() != 4 {
            return Err(Error::Unsupported);
        }
        if !(1..=6).contains(&self.owner) {
            return Err(Error::Row);
        }
        Ok(crate::business_takeover::Command {
            building: crate::player_manager::word(&self.payload, 0),
        })
    }
    pub fn terms(serial: u64, owner: u8, command: &crate::economy::PendingBusinessTerms) -> Self {
        let mut payload = command.business_index.to_le_bytes().to_vec();
        for value in command.terms.stock_targets {
            payload.extend_from_slice(&value.to_le_bytes());
        }
        for value in command.terms.prices {
            payload.extend_from_slice(&value.to_le_bytes());
        }
        payload.extend_from_slice(&command.terms.rent.to_le_bytes());
        Self {
            serial,
            owner,
            kind: 0x18,
            payload,
        }
    }
    pub fn terms_payload(&self) -> Result<crate::economy::PendingBusinessTerms> {
        if self.kind != 0x18 || self.payload.len() != 44 {
            return Err(Error::Unsupported);
        }
        let command = crate::economy::PendingBusinessTerms {
            business_index: crate::player_manager::word(&self.payload, 0),
            terms: crate::economy::BusinessTerms {
                stock_targets: std::array::from_fn(|i| {
                    i16::from_le_bytes([self.payload[4 + i * 2], self.payload[5 + i * 2]])
                }),
                prices: std::array::from_fn(|i| {
                    f32::from_bits(crate::player_manager::word(&self.payload, 16 + i * 4))
                }),
                rent: f32::from_bits(crate::player_manager::word(&self.payload, 40)),
            },
        };
        if !(1..=6).contains(&self.owner)
            || !command.terms.rent.is_finite()
            || command.terms.prices.iter().any(|price| !price.is_finite())
        {
            return Err(Error::Row);
        }
        Ok(command)
    }
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
        if self.is_auction() {
            return self.auction_payload().map(|_| ());
        }
        match self.kind {
            0x0f => self.construction_payload().map(|_| ()),
            0x10 => self.demolition_payload().map(|_| ()),
            0x11 => self.business_open_payload().map(|_| ()),
            0x12 => self.conversion_payload().map(|_| ()),
            0x13 => self.business_sale_payload().map(|_| ()),
            0x15 => self.takeover_payload().map(|_| ()),
            0x18 => self.terms_payload().map(|_| ()),
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

/// Read-only target records borrowed from the actual Session, including when
/// AI dispatch already holds the mutable category and business fields separately.
#[derive(Clone, Copy)]
pub struct Targets<'a> {
    pub buildings: &'a [[u8; 0x394]],
    pub businesses: &'a [[u8; 0x220]],
    /// Copied from the live categories before borrowing their mutable occupancy.
    pub category_owners: [u8; 52],
}
impl Default for Targets<'_> {
    fn default() -> Self {
        Self {
            buildings: &[],
            businesses: &[],
            category_owners: [0; 52],
        }
    }
}
impl<'a> Targets<'a> {
    pub fn new(
        buildings: &'a [[u8; 0x394]],
        businesses: &'a [[u8; 0x220]],
        categories: &[[u8; 0x17c]; 52],
    ) -> Self {
        Self {
            buildings,
            businesses,
            category_owners: categories.map(|row| row[5]),
        }
    }
    fn business(&self, id: u32) -> Result<&[u8; 0x220]> {
        let row = self.businesses.get(id as usize).ok_or(Error::Row)?;
        if row[0x21c] == 0 {
            return Err(Error::Row);
        }
        let building_id = u32::from(u16::from_le_bytes([row[8], row[9]]));
        let building = self.building(building_id)?;
        if crate::player_manager::word(building, 0x14) != 4
            || crate::player_manager::word(building, 0x1c) != id
        {
            return Err(Error::Row);
        }
        Ok(row)
    }
    fn linked_business(&self, building: u32) -> Result<&[u8; 0x220]> {
        let row = self.building(building)?;
        let business = self.business(crate::player_manager::word(row, 0x1c))?;
        if u32::from(u16::from_le_bytes([business[8], business[9]])) != building {
            return Err(Error::Row);
        }
        Ok(business)
    }
    fn takeover_owner(&self, command: &crate::business_takeover::Command) -> Result<u8> {
        let row = self.building(command.building)?;
        let business = self.linked_business(command.building)?;
        // 451264 derives the buyer from the category landlord, not either
        // building/business owner and not a sender field in the native payload.
        let offset = crate::player_manager::word(row, 0x40)
            .checked_sub(0x617628)
            .ok_or(Error::Row)?;
        if !offset.is_multiple_of(380) {
            return Err(Error::Row);
        }
        let owner = *self
            .category_owners
            .get(offset as usize / 380)
            .ok_or(Error::Row)?;
        if !(1..=6).contains(&owner) || business[0xa] == owner {
            return Err(Error::Row);
        }
        Ok(owner)
    }
    fn building(&self, id: u32) -> Result<&[u8; 0x394]> {
        let row = self.buildings.get(id as usize).ok_or(Error::Row)?;
        if row[0x3f] == 0 || crate::player_manager::word(row, 0) != id {
            return Err(Error::Row);
        }
        Ok(row)
    }
    fn demolition_owner(&self, command: &crate::demolition::Command) -> Result<u8> {
        let row = self.building(u32::from(command.building))?;
        let category = crate::player_manager::word(row, 0x40);
        if command.category < 0
            || category != 0x617628 + command.category as u32 * 380
            || crate::player_manager::word(row, 0x14) != 4
        {
            return Err(Error::Row);
        }
        // 451458 debits the linked business owner, not the category landlord.
        let business = crate::player_manager::word(row, 0x1c);
        if business == u32::MAX {
            return Ok(row[0x2d]);
        }
        let business = self.businesses.get(business as usize).ok_or(Error::Row)?;
        if business[0x21c] == 0 {
            return Err(Error::Row);
        }
        Ok(business[0xa])
    }
    fn check(&self, request: &Request) -> Result<()> {
        match request.kind {
            0x10 => {
                if self.demolition_owner(&request.demolition_payload()?)? != request.owner {
                    return Err(Error::Peer);
                }
            }
            0x11 => {
                let command = request.business_open_payload()?;
                let row = self.building(command.building)?;
                // Completion/AI opening acts for the building's gameplay owner.
                if row[0x2d] != request.owner {
                    return Err(Error::Peer);
                }
                if crate::player_manager::word(row, 0x14) != 4
                    || crate::player_manager::word(row, 0x1c) != u32::MAX
                {
                    return Err(Error::Row);
                }
            }
            0x12 => {
                let command = request.conversion_payload()?;
                let business = self.linked_business(command.building)?;
                if business[0xa] != request.owner {
                    return Err(Error::Peer);
                }
                if crate::player_manager::word(business, 4) == command.kind {
                    return Err(Error::Row);
                }
            }
            0x13 => {
                let command = request.business_sale_payload()?;
                if self.business(command.business)?[0xa] != request.owner {
                    return Err(Error::Peer);
                }
            }
            0x15 => {
                if self.takeover_owner(&request.takeover_payload()?)? != request.owner {
                    return Err(Error::Peer);
                }
            }
            0x18 => {
                let command = request.terms_payload()?;
                if self.business(command.business_index)?[0xa] != request.owner {
                    return Err(Error::Peer);
                }
            }
            _ => (),
        }
        Ok(())
    }
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
    pending_local: std::collections::VecDeque<(Request, bool)>,
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
        if request.is_auction() {
            return self.authorize_auction(peer, request);
        }
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
    pub fn request(
        &mut self,
        owner: &Owner,
        request: Request,
        tick: u32,
        targets: Targets<'_>,
    ) -> Result<Message> {
        owner.validate_binding(self.binding)?;
        self.authorize(self.local_peer, &request)?;
        targets.check(&request)?;
        if self.host && self.closed_tick == Some(tick) {
            return Err(Error::Phase);
        }
        if self.host {
            self.commit(self.local_peer, request, tick)
        } else {
            Ok(Message::CommandRequest(request))
        }
    }
    fn next_serial(&mut self) -> Result<u64> {
        let serial = self.next_request;
        self.next_request = serial.checked_add(1).ok_or(Error::Exhausted)?;
        Ok(serial)
    }
    pub fn next_transfer(&mut self, transfer: Transfer) -> Result<Request> {
        let serial = self.next_serial()?;
        Ok(Request::transfer(serial, transfer))
    }
    pub fn next_construction(&mut self, command: &crate::construction::Command) -> Result<Request> {
        let serial = self.next_serial()?;
        Ok(Request::construction(serial, command))
    }
    fn commit(&mut self, peer: u8, request: Request, tick: u32) -> Result<Message> {
        self.authorize(peer, &request)?;
        self.commit_authorized(peer, request, tick)
    }
    fn commit_authorized(&mut self, peer: u8, request: Request, tick: u32) -> Result<Message> {
        // 451720 adds authority Total+25 and any announcement delay once.
        // Flag-zero auction actions retain zero, the native immediate sentinel.
        let due = request.command_due(tick)?;
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
    pub fn receive(
        &mut self,
        owner: &Owner,
        ingress: Ingress,
        tick: u32,
        targets: Targets<'_>,
    ) -> Result<Message> {
        if ingress.binding != self.binding {
            return Err(Error::World);
        }
        owner.validate_connection(self.binding, ingress.connection, !self.host)?;
        match ingress.message {
            Message::CommandRequest(request) if self.host => {
                targets.check(&request)?;
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
    #[allow(clippy::too_many_arguments)]
    pub fn submit(
        &mut self,
        worker: &super::lobby::worker::Handle,
        command: crate::live_commands::Command,
        queue: &mut crate::live_commands::Queue,
        categories: &mut [[u8; 0x17c]; 52],
        targets: Targets<'_>,
        tick: u32,
    ) -> std::result::Result<(), String> {
        let request = match command {
            crate::live_commands::Command::Auction { descriptor } => {
                return self.submit_auction(worker, descriptor, 0, queue, categories, tick);
            }
            crate::live_commands::Command::Build(command) => self.next_construction(&command),
            crate::live_commands::Command::Demolish(command) => {
                let owner = targets
                    .demolition_owner(&command)
                    .map_err(|e| e.to_string())?;
                let serial = self.next_serial().map_err(|e| e.to_string())?;
                Ok(Request::demolition(serial, owner, &command))
            }
            crate::live_commands::Command::Open(command) => self
                .next_serial()
                .map(|serial| Request::business_open(serial, &command)),
            crate::live_commands::Command::Convert(command) => self
                .next_serial()
                .map(|serial| Request::conversion(serial, &command)),
            crate::live_commands::Command::SellBusiness(command) => self
                .next_serial()
                .map(|serial| Request::business_sale(serial, &command)),
            crate::live_commands::Command::Takeover(command) => {
                let owner = targets
                    .takeover_owner(&command)
                    .map_err(|e| e.to_string())?;
                self.next_serial()
                    .map(|serial| Request::takeover(serial, owner, &command))
            }
            crate::live_commands::Command::Terms(command) => {
                let owner = targets
                    .business(command.business_index)
                    .map_err(|e| e.to_string())?[0xa];
                self.next_serial()
                    .map(|serial| Request::terms(serial, owner, &command))
            }
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
        self.submit_request(worker, request, false, queue, categories, targets, tick)
    }
    /// Only the authority's pending-world transition generates completions,
    /// including buildings owned by remote humans. Network requests still
    /// require the sending connection to own the command's gameplay owner.
    #[cfg(feature = "lua4")]
    #[allow(clippy::too_many_arguments)]
    pub fn submit_completion(
        &mut self,
        worker: &super::lobby::worker::Handle,
        command: &crate::business_open::Command,
        queue: &mut crate::live_commands::Queue,
        categories: &mut [[u8; 0x17c]; 52],
        targets: Targets<'_>,
        tick: u32,
    ) -> std::result::Result<(), String> {
        let serial = self.next_serial().map_err(|e| e.to_string())?;
        self.submit_request(
            worker,
            Request::business_open(serial, command),
            true,
            queue,
            categories,
            targets,
            tick,
        )
    }
    #[cfg(feature = "lua4")]
    fn authorize_local(&self, request: &Request, completion: bool) -> Result<()> {
        if !completion {
            return self.authorize(self.local_peer, request);
        }
        if !self.host || request.kind != 0x11 {
            return Err(Error::Peer);
        }
        request.validate()?;
        if self
            .owners
            .get(usize::from(request.owner).wrapping_sub(1))
            .is_none()
        {
            return Err(Error::Row);
        }
        if self.requests[0].is_some_and(|last| request.serial <= last) {
            return Err(Error::Sequence);
        }
        Ok(())
    }
    #[cfg(feature = "lua4")]
    #[allow(clippy::too_many_arguments)]
    fn submit_request(
        &mut self,
        worker: &super::lobby::worker::Handle,
        request: Request,
        completion: bool,
        queue: &mut crate::live_commands::Queue,
        categories: &mut [[u8; 0x17c]; 52],
        targets: Targets<'_>,
        tick: u32,
    ) -> std::result::Result<(), String> {
        worker.validate_binding(self.binding)?;
        self.authorize_local(&request, completion)
            .map_err(|e| e.to_string())?;
        targets.check(&request).map_err(|e| e.to_string())?;
        if self.host
            && ((self.closed_tick == Some(tick)
                && request.command_due(tick).map_err(|e| e.to_string())? != 0)
                || !self.pending_local.is_empty())
        {
            if self.pending_local.len() >= 64 {
                return Err("Authority command queue is full".into());
            }
            self.pending_local.push_back((request, completion));
            return Ok(());
        }
        check_placement(&request, categories).map_err(|e| e.to_string())?;
        let message = if self.host {
            self.commit_authorized(self.local_peer, request, tick)
                .map_err(|e| e.to_string())?
        } else {
            Message::CommandRequest(request)
        };
        if let crate::network::Message::CommandCommit(commit) = &message {
            self.enqueue(commit, queue, categories, tick)
                .map_err(|e| e.to_string())?;
        }
        worker.send_gameplay(self.binding, None, message)
    }
    /// Ordered ingress, kind8 resume and five-Hz authority deadlines. Borrow the
    /// actual Session queue/pause/tasks; no replacement timer records are made.
    #[cfg(feature = "lua4")]
    #[allow(clippy::too_many_arguments)]
    pub fn pump(
        &mut self,
        worker: &super::lobby::worker::Handle,
        tick: u32,
        queue: &mut crate::live_commands::Queue,
        categories: &mut [[u8; 0x17c]; 52],
        targets: Targets<'_>,
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
        while let Some((request, completion)) = self.pending_local.pop_front() {
            self.authorize_local(&request, completion)
                .map_err(|e| e.to_string())?;
            targets.check(&request).map_err(|e| e.to_string())?;
            check_placement(&request, categories).map_err(|e| e.to_string())?;
            let message = self
                .commit_authorized(self.local_peer, request, tick)
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
                .and_then(|()| self.receive(worker.gameplay_owner(), ingress, tick, targets))
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
        if commit.order != expected
            || (commit.due != 0 && commit.due <= tick)
            || commit.due == u32::MAX
        {
            return Err(Error::Sequence);
        }
        let request = &commit.request;
        request.validate()?;
        if commit.due == 0 && !request.is_auction() {
            return Err(Error::Sequence);
        }
        if request.is_auction() {
            let (d, _) = request.auction_payload()?;
            if (d[16] == 0) != (commit.due == 0) {
                return Err(Error::Row);
            }
        }
        if !request.is_auction()
            && self
                .owners
                .get(usize::from(request.owner).wrapping_sub(1))
                .is_none()
        {
            return Err(Error::Row);
        }
        let next = self.next_enqueued.checked_add(1).ok_or(Error::Exhausted)?;
        let command = if request.is_auction() {
            self.enqueue_auction(request, commit.due)?
        } else {
            match request.kind {
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
                0x10 => crate::live_commands::Command::Demolish(request.demolition_payload()?),
                0x11 => crate::live_commands::Command::Open(request.business_open_payload()?),
                0x12 => crate::live_commands::Command::Convert(request.conversion_payload()?),
                0x13 => {
                    crate::live_commands::Command::SellBusiness(request.business_sale_payload()?)
                }
                0x15 => crate::live_commands::Command::Takeover(request.takeover_payload()?),
                0x18 => crate::live_commands::Command::Terms(request.terms_payload()?),
                0x16 => {
                    let t = request.transfer_payload()?;
                    crate::live_commands::Command::PropertyTransfer {
                        category: i16::from(t.category),
                        owner: t.owner,
                        amount: t.amount,
                    }
                }
                _ => return Err(Error::Unsupported),
            }
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
