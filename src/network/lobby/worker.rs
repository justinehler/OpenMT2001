//! UI adapter. Only `spawn`'s worker performs listen/connect/pump/send; the UI
//! uses bounded try_send/try_recv. Drop retires the captured attempt immediately,
//! so a slow connect completion cannot install into or retire its replacement.
use super::{Action, Request, Snapshot};
use crate::network::{self, Attempt, Connection, Message, Owner, SLOTS, transport::Event};
use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    pub address: SocketAddr,
    pub session: [u8; 16],
    pub snapshot: Snapshot,
}
#[derive(Clone, Debug)]
pub struct View {
    pub snapshot: Snapshot,
    pub local_peer: u8,
    pub host: bool,
    pub address: SocketAddr,
}
#[derive(Clone, Debug)]
pub enum Update {
    Probed(Selection),
    Lobby(View),
    Notice(String),
    Failed(String),
}
pub enum Open {
    Host {
        address: SocketAddr,
        snapshot: Snapshot,
        token_names: [Vec<u8>; 10],
    },
    Probe(SocketAddr),
    Join {
        selection: Selection,
        name: Vec<u8>,
        token: i8,
    },
}
pub struct Handle {
    owner: &'static Owner,
    attempt: Attempt,
    cancel: std::sync::Arc<AtomicBool>,
    commands: SyncSender<Action>,
    updates: Receiver<Update>,
}
impl Handle {
    pub fn open(open: Open) -> Result<Self, String> {
        Self::spawn(network::process(), open)
    }
    fn spawn(owner: &'static Owner, open: Open) -> Result<Self, String> {
        let attempt = owner.begin().map_err(|e| e.to_string())?;
        let cancel = std::sync::Arc::new(AtomicBool::new(false));
        let stopped = cancel.clone();
        let (commands, actions) = mpsc::sync_channel(32);
        let (publish, updates) = mpsc::sync_channel(16);
        if let Err(error) = thread::Builder::new()
            .name("multiplayer lobby".into())
            .spawn(move || {
                let _retire = Retirement { owner, attempt };
                if let Err(error) = run(owner, attempt, open, actions, &publish, &stopped) {
                    // Bounded error delivery also remains off the UI thread.
                    while !stopped.load(Ordering::Relaxed) {
                        match publish.try_send(Update::Failed(error.clone())) {
                            Err(TrySendError::Full(_)) => thread::sleep(Duration::from_millis(10)),
                            _ => break,
                        }
                    }
                }
            })
        {
            let _ = owner.retire(attempt);
            return Err(error.to_string());
        }
        Ok(Self {
            owner,
            attempt,
            cancel,
            commands,
            updates,
        })
    }
    pub fn action(&self, action: Action) -> Result<(), String> {
        self.commands
            .try_send(action)
            .map_err(|_| "Lobby is busy; try again.".into())
    }
    pub fn poll(&self) -> Option<Update> {
        self.updates.try_recv().ok()
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        let _ = self.owner.retire(self.attempt);
    }
}
struct Retirement {
    owner: &'static Owner,
    attempt: Attempt,
}
impl Drop for Retirement {
    fn drop(&mut self) {
        let _ = self.owner.retire(self.attempt);
    }
}
fn session(owner: &Owner, attempt: Attempt) -> Result<[u8; 16], String> {
    let state = owner.lock().map_err(|e| e.to_string())?;
    state.attempt(attempt).map_err(|e| e.to_string())?;
    state.session.ok_or_else(|| "Session has closed.".into())
}
fn run(
    owner: &Owner,
    attempt: Attempt,
    open: Open,
    actions: Receiver<Action>,
    publish: &SyncSender<Update>,
    cancel: &AtomicBool,
) -> Result<(), String> {
    if let Open::Probe(address) = &open {
        let (session, snapshot) = network::transport::probe(*address, Duration::from_secs(3))
            .map_err(|e| e.to_string())?;
        if !cancel.load(Ordering::Relaxed) {
            publish
                .try_send(Update::Probed(Selection {
                    address: *address,
                    session,
                    snapshot,
                }))
                .map_err(|_| "Probe receiver closed".to_string())?;
        }
        return Ok(());
    }
    let (host, address) = match &open {
        Open::Host { address, .. } => (true, *address),
        Open::Probe(address) => (false, *address),
        Open::Join { selection, .. } => (false, selection.address),
    };
    let address = if host {
        let mut id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
            .to_le_bytes();
        // Process-local attempt identity also separates rapid rehosting.
        for (i, b) in attempt.number.to_le_bytes().iter().enumerate() {
            id[i] ^= b;
        }
        owner
            .listen(attempt, address, id)
            .map_err(|e| e.to_string())?
    } else {
        // STRUCTURAL GUARANTEE: run is invoked only inside thread::spawn above.
        owner
            .connect(attempt, address, Duration::from_secs(3))
            .map_err(|e| e.to_string())?;
        address
    };
    let mut snapshot = match &open {
        Open::Host { snapshot, .. } => Some(snapshot.clone()),
        _ => None,
    };
    if let Some(snapshot) = &snapshot {
        owner
            .advertise(attempt, snapshot.clone())
            .map_err(|e| e.to_string())?;
    }
    let mut peers: [Option<Connection>; SLOTS] = [None; SLOTS];
    let mut sent_revisions = [None; SLOTS];
    let mut local_peer = 0;
    let mut requested_join = false;
    let mut admitted = host;
    let deadline = Instant::now() + Duration::from_secs(12);
    let mut pending = snapshot.clone().map(|snapshot| {
        Update::Lobby(View {
            snapshot,
            local_peer,
            host,
            address,
        })
    });
    while !cancel.load(Ordering::Relaxed) {
        if !admitted && Instant::now() > deadline {
            return Err("Timed out waiting for session information or lobby admission.".into());
        }
        let mut changed = false;
        for event in owner.pump(attempt, None).map_err(|e| e.to_string())? {
            match event {
                Event::Connected {
                    connection,
                    local_slot,
                } => {
                    peers[connection.slot() as usize] = Some(connection);
                    if host {
                        sent_revisions[connection.slot() as usize] = None;
                    } else {
                        local_peer = local_slot;
                    }
                }
                Event::Message {
                    connection,
                    message,
                } => match *message {
                    Message::LobbyRequest(request) if host => {
                        let roster = snapshot.as_mut().unwrap();
                        if roster.request(connection.slot(), request).is_ok() {
                            changed = true;
                        } else {
                            match owner.send(connection, Message::LobbyDenied) {
                                Ok(()) | Err(network::transport::Error::QueueFull) => (),
                                Err(error) => return Err(error.to_string()),
                            }
                        }
                    }
                    Message::Lobby(next) if !host => {
                        if let Open::Join {
                            selection,
                            name,
                            token,
                        } = &open
                        {
                            if session(owner, attempt)? != selection.session
                                || next.settings != selection.snapshot.settings
                            {
                                return Err(
                                    "The selected session changed. Probe the address again.".into(),
                                );
                            }
                            if !requested_join {
                                if next.capacity().0 >= next.capacity().1 {
                                    return Err(
                                        "The selected session has no open human slots.".into()
                                    );
                                }
                                owner
                                    .send(
                                        connection,
                                        Message::LobbyRequest(Request::Join {
                                            name: name.clone(),
                                            token: *token,
                                        }),
                                    )
                                    .map_err(|e| e.to_string())?;
                                requested_join = true;
                            }
                        }
                        if let Some(old) = &snapshot {
                            if next == *old {
                                continue;
                            }
                            if next.revision <= old.revision {
                                return Err("Invalid lobby revision.".into());
                            }
                        }
                        admitted = next.local_row(local_peer).is_some();
                        snapshot = Some(next);
                        changed = admitted;
                    }
                    Message::LobbyDenied if !host => {
                        if !admitted {
                            return Err("Host refused the join request; probe again.".into());
                        }
                        pending = Some(Update::Notice(
                            "Host refused the change; the roster may have changed.".into(),
                        ));
                    }
                    _ => return Err("Unexpected message in lobby.".into()),
                },
                Event::Failed { connection, error } => {
                    if !host {
                        return Err(format!(
                            "Host connection ended: {error}. Host migration is not available."
                        ));
                    }
                    if let Some(connection) = connection {
                        peers[connection.slot() as usize] = None;
                        let roster = snapshot.as_mut().unwrap();
                        let revision = roster.revision;
                        roster
                            .remove(connection.slot())
                            .map_err(|e| e.to_string())?;
                        changed |= roster.revision != revision;
                    }
                }
            }
        }
        for action in actions.try_iter().take(32) {
            let Some(roster) = snapshot.as_mut().filter(|_| admitted) else {
                continue;
            };
            if host {
                if action == Action::Start {
                    pending = Some(Update::Notice(if roster.start_enabled() {
                        super::START_BOUNDARY.into()
                    } else {
                        "At least two populated rows must have a character, name and readiness."
                            .into()
                    }));
                } else if let Open::Host { token_names, .. } = &open {
                    match roster.action(action, token_names) {
                        Ok(()) => changed = true,
                        Err(error) => pending = Some(Update::Notice(error.to_string())),
                    }
                }
            } else {
                let request = roster
                    .client_edit(local_peer, action)
                    .map_err(|e| e.to_string())?;
                owner
                    .send(
                        peers[0].ok_or("Host connection missing")?,
                        Message::LobbyRequest(request),
                    )
                    .map_err(|e| e.to_string())?;
            }
        }
        if changed {
            let roster = snapshot.as_ref().unwrap();
            if host {
                owner
                    .advertise(attempt, roster.clone())
                    .map_err(|e| e.to_string())?;
            }
            pending = Some(Update::Lobby(View {
                snapshot: roster.clone(),
                local_peer,
                host,
                address,
            }));
        }
        if host {
            let roster = snapshot.as_ref().unwrap();
            for (slot, connection) in peers
                .iter()
                .enumerate()
                .filter_map(|(i, c)| c.map(|c| (i, c)))
            {
                if sent_revisions[slot] == Some(roster.revision) {
                    continue;
                }
                match owner.send(connection, Message::Lobby(roster.clone())) {
                    Ok(()) => sent_revisions[slot] = Some(roster.revision),
                    // Keep the latest revision pending until this peer drains;
                    // a slow receiver cannot retire all the other lobby peers.
                    Err(network::transport::Error::QueueFull) => (),
                    Err(error) => return Err(error.to_string()),
                }
            }
        }
        if let Some(update) = pending.take() {
            match publish.try_send(update) {
                Ok(()) => (),
                Err(TrySendError::Full(update)) => pending = Some(update),
                Err(TrySendError::Disconnected(_)) => break,
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}
#[cfg(test)]
mod tests;
