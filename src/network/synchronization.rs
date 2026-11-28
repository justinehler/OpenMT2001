//! Retained 5 Hz maintenance, reason-2 handshake and diagnostic samples.
//! References: 0x450210/340/390/410, 0x481340/520. Samples never repair state.
use super::{Error, Message, Result, SLOTS, SessionBinding};

pub mod sample;
pub use sample::{Comparison, Comparisons, Field, Mismatch, Sample, Status};

#[derive(Debug)]
pub struct Ingress {
    pub(super) binding: SessionBinding,
    pub(super) connection: super::Connection,
    pub(super) message: Message,
}

pub struct Synchronization {
    binding: SessionBinding,
    host: bool,
    local_slot: u8,
    peers: Vec<Option<u8>>,
    pub barrier: Barrier,
    pub samples: sample::Samples,
    task: Option<usize>,
}
impl Synchronization {
    pub fn new(
        binding: SessionBinding,
        host: bool,
        local_peer: u8,
        peers: Vec<Option<u8>>,
    ) -> Result<Self> {
        if peers.is_empty()
            || peers.len() > SLOTS
            || peers.first() != Some(&Some(0))
            || host != (local_peer == 0)
        {
            return Err(Error::Peer);
        }
        let mut seen = [false; SLOTS];
        for peer in peers.iter().flatten().copied() {
            let entry = seen.get_mut(usize::from(peer)).ok_or(Error::Peer)?;
            if *entry {
                return Err(Error::Peer);
            }
            *entry = true;
        }
        let local_slot = peers
            .iter()
            .position(|p| *p == Some(local_peer))
            .ok_or(Error::Peer)? as u8;
        Ok(Self {
            binding,
            host,
            local_slot,
            peers,
            barrier: Barrier::default(),
            samples: Default::default(),
            task: None,
        })
    }
    /// 0x450430 registers once or enables the same group-0 record.
    pub fn enable(&mut self, tasks: &mut Vec<crate::timers::Task>) -> Result<usize> {
        if let Some(index) = self.task {
            let task = tasks.get_mut(index).ok_or(Error::World)?;
            if u32::from_le_bytes(task[16..20].try_into().map_err(|_| Error::World)?) != 0x450410 {
                return Err(Error::World);
            }
            task[0] = 1;
            return Ok(index);
        }
        let mut task = [0; 20];
        crate::timers::configure(&mut task, 5., 0x450410);
        let index = tasks.len();
        tasks.push(task);
        self.task = Some(index);
        Ok(index)
    }
    /// 0x465490 opens reason 2 only after the local scene and activation have
    /// completed. Room exit (0x464bf0) has no second handshake in the original.
    pub fn auction_prepared(
        &mut self,
        pause: &mut crate::pause::State,
        tasks: &mut [crate::timers::Task],
    ) -> std::result::Result<(), String> {
        self.barrier.open(pause, tasks)
    }
    fn sender(&self, owner: &super::Owner, ingress: &Ingress, slot: u8) -> Result<()> {
        if ingress.binding != self.binding {
            return Err(Error::World);
        }
        owner.validate_connection(self.binding, ingress.connection, !self.host)?;
        if self.peers.get(usize::from(slot)) != Some(&Some(ingress.connection.slot())) {
            return Err(Error::Peer);
        }
        Ok(())
    }
    pub fn receive(
        &mut self,
        owner: &super::Owner,
        ingress: Ingress,
        pause: &mut crate::pause::State,
        tasks: &mut [crate::timers::Task],
    ) -> std::result::Result<bool, String> {
        let slot = match &ingress.message {
            Message::Barrier { sender, .. } => *sender,
            Message::StateSample(sample) => sample.sender,
            _ => return Err(Error::Tag.to_string()),
        };
        self.sender(owner, &ingress, slot)
            .map_err(|e| e.to_string())?;
        match ingress.message {
            Message::Barrier { sender, complete } => self
                .barrier
                .receive(self.host, sender, complete, pause, tasks),
            Message::StateSample(sample) if self.host => {
                self.samples.compare(sample);
                Ok(false)
            }
            _ => Err(Error::Peer.to_string()),
        }
    }
    #[cfg(feature = "lua4")]
    pub fn capture(
        &mut self,
        worker: &super::lobby::worker::Handle,
        tick: u32,
        city: &mut crate::level_startup::City,
        roads: &crate::traffic::road::runtime::RoadTraffic,
        players: &[crate::player_manager::Player],
    ) -> std::result::Result<(), String> {
        let sample = sample::capture(self.local_slot, tick, city, roads, players)?;
        self.samples.retain(sample);
        if !self.host {
            worker.send_synchronization(self.binding, None, Message::StateSample(sample))?;
        }
        Ok(())
    }
    /// Receive and loss admission precede barrier work even when reason 2 has
    /// disabled every group-2 task. Return true when activity time must sync.
    pub fn pump(
        &mut self,
        worker: &super::lobby::worker::Handle,
        active_humans: [bool; SLOTS],
        pause: &mut crate::pause::State,
        tasks: &mut [crate::timers::Task],
    ) -> std::result::Result<bool, String> {
        worker.validate_binding(self.binding)?;
        while let Some(update) = worker.poll() {
            if let super::lobby::worker::Update::Failed(error) = update {
                return Err(error);
            }
        }
        let mut closed = false;
        while let Some(ingress) = worker.poll_synchronization() {
            closed |= self.receive(worker.gameplay_owner(), ingress, pause, tasks)?;
        }
        closed |= self.barrier.pump(
            self.host,
            self.local_slot,
            active_humans,
            pause,
            tasks,
            |target, complete| {
                let peer = match target {
                    Some(slot) => match self.peers[usize::from(slot)] {
                        Some(peer) => Some(peer),
                        None => return Ok(false),
                    },
                    None => None,
                };
                worker.send_synchronization(
                    self.binding,
                    peer,
                    Message::Barrier {
                        sender: self.local_slot,
                        complete,
                    },
                )
            },
        )?;
        Ok(closed)
    }
}

#[derive(Default)]
pub struct Barrier {
    pub active: bool,
    acknowledgments: [bool; SLOTS],
    phase: u8,
    /// Original signed global; opening does not reset its phase.
    counter: i32,
}
impl Barrier {
    pub fn open(
        &mut self,
        pause: &mut crate::pause::State,
        tasks: &mut [crate::timers::Task],
    ) -> std::result::Result<(), String> {
        self.active = true;
        self.acknowledgments.fill(false);
        self.phase = 0;
        pause.pause(2, |change, _| {
            change.apply(tasks);
            Ok(())
        })
    }
    fn close(
        &mut self,
        pause: &mut crate::pause::State,
        tasks: &mut [crate::timers::Task],
    ) -> std::result::Result<bool, String> {
        self.active = false;
        pause.unpause(2, |change, _| {
            change.apply(tasks);
            Ok(())
        })?;
        Ok(true)
    }
    pub fn receive(
        &mut self,
        host: bool,
        sender: u8,
        complete: u8,
        pause: &mut crate::pause::State,
        tasks: &mut [crate::timers::Task],
    ) -> std::result::Result<bool, String> {
        if usize::from(sender) >= SLOTS {
            return Err(Error::Peer.to_string());
        }
        if !self.active {
            return Ok(false);
        }
        if host {
            self.acknowledgments[usize::from(sender)] = true;
        } else if complete != 0 {
            return self.close(pause, tasks);
        } else {
            self.phase = 1;
        }
        Ok(false)
    }
    pub fn pump(
        &mut self,
        host: bool,
        local_slot: u8,
        active_humans: [bool; SLOTS],
        pause: &mut crate::pause::State,
        tasks: &mut [crate::timers::Task],
        mut send: impl FnMut(Option<u8>, u8) -> std::result::Result<bool, String>,
    ) -> std::result::Result<bool, String> {
        if !self.active {
            return Ok(false);
        }
        if usize::from(local_slot) >= SLOTS {
            return Err(Error::Peer.to_string());
        }
        self.counter = self.counter.wrapping_add(1);
        if self.counter % 5 != 0 {
            return Ok(false);
        }
        if host {
            self.acknowledgments[usize::from(local_slot)] = true;
            let mut pending = 0;
            for (slot, active) in active_humans.into_iter().enumerate() {
                if active && !self.acknowledgments[slot] && send(Some(slot as u8), 0)? {
                    pending += 1;
                }
            }
            if pending == 0 {
                let _ = send(None, 1)?;
                return self.close(pause, tasks);
            }
        } else if self.phase == 1 {
            let _ = send(None, 0)?;
            self.phase = 2;
        }
        Ok(false)
    }
}

/// Native six pointer slots: a live human is present, active at +10, and kind
/// zero at +154. Read these bytes again for each maintenance callback.
pub fn active_humans(players: &[crate::player_manager::Player]) -> [bool; SLOTS] {
    std::array::from_fn(|slot| {
        players.get(slot).is_some_and(|p| {
            p.bytes.get(0x10).is_some_and(|b| *b != 0)
                && p.bytes.get(0x154..0x158).is_some_and(|b| b == [0; 4])
        })
    })
}

#[cfg(test)]
mod tests;
