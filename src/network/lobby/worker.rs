//! UI adapter. The worker performs listen/connect/pump and command sends; the UI
//! uses bounded channels. Synchronization queues directly for real send results.
//! Drop retires the captured attempt immediately,
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
    Prepare(Ticket),
    Validated,
    Probed(Selection),
    Lobby(View),
    Notice(String),
    Failed(String),
}
/// Frozen admitted lobby and the authority seed. Only the worker mints this.
#[derive(Clone, Debug)]
pub struct Ticket {
    pub view: View,
    pub seed: u32,
    pub expected: Option<network::Content>,
}
enum Command {
    Chat(Vec<u8>, Option<network::chat::Recipients>),
    Gameplay(network::SessionBinding, Option<Connection>, Box<Message>),
    Stage(Box<dyn FnOnce() -> Result<network::Content, String> + Send>),
    Edit(Action),
    Prepared(std::result::Result<network::Content, String>),
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
pub struct Cancellation {
    owner: &'static Owner,
    attempt: Attempt,
    cancel: std::sync::Arc<AtomicBool>,
}
impl Cancellation {
    pub fn cancel(self) {
        self.cancel.store(true, Ordering::Relaxed);
        let _ = self.owner.retire(self.attempt);
    }
}
pub struct Handle {
    chat: std::sync::Arc<std::sync::Mutex<network::chat::Display>>,
    owner: &'static Owner,
    attempt: Attempt,
    cancel: std::sync::Arc<AtomicBool>,
    commands: SyncSender<Command>,
    updates: Receiver<Update>,
    gameplay: Receiver<network::commands::Ingress>,
    synchronization: Receiver<network::synchronization::Ingress>,
}
impl Handle {
    pub fn open(open: Open) -> Result<Self, String> {
        Self::spawn(network::process(), open)
    }
    fn spawn(owner: &'static Owner, open: Open) -> Result<Self, String> {
        Self::spawn_with_timeout(owner, open, Duration::from_secs(3))
    }
    fn spawn_with_timeout(
        owner: &'static Owner,
        open: Open,
        network_timeout: Duration,
    ) -> Result<Self, String> {
        let chat = std::sync::Arc::new(std::sync::Mutex::new(network::chat::Display::new()));
        let worker_chat = chat.clone();
        let attempt = owner.begin().map_err(|e| e.to_string())?;
        let cancel = std::sync::Arc::new(AtomicBool::new(false));
        let stopped = cancel.clone();
        let (commands, actions) = mpsc::sync_channel(32);
        let (publish, updates) = mpsc::sync_channel(16);
        let (gameplay_tx, gameplay) = mpsc::sync_channel(64);
        let (synchronization_tx, synchronization) = mpsc::sync_channel(64);
        if let Err(error) = thread::Builder::new()
            .name("multiplayer lobby".into())
            .spawn(move || {
                let _retire = Retirement { owner, attempt };
                if let Err(error) = run(
                    owner,
                    attempt,
                    open,
                    actions,
                    Publications {
                        lobby: &publish,
                        gameplay: &gameplay_tx,
                        synchronization: &synchronization_tx,
                        chat: &worker_chat,
                    },
                    &stopped,
                    network_timeout,
                ) {
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
            chat,
            owner,
            attempt,
            cancel,
            commands,
            updates,
            gameplay,
            synchronization,
        })
    }
    /// The retained chat lines, named and coloured from the current admitted roster.
    /// `None` until this peer is admitted.
    pub fn chat_view(&self) -> Option<network::chat::View> {
        self.chat.lock().ok()?.view()
    }
    /// Submits one chat line: group delivery with `None` (lobby), or the selected
    /// recipients (in-game Chat panel). Returns false, sending and echoing nothing, for
    /// empty text or a selection with no recipient other than this player. Otherwise the
    /// worker sends it and appends the one local echo. A full command queue also
    /// returns false.
    pub fn submit_chat(
        &self,
        text: Vec<u8>,
        recipients: Option<network::chat::Recipients>,
    ) -> Result<bool, String> {
        self.validate()?;
        let view = self.chat_view().ok_or("Chat has no admitted lobby")?;
        let local = view
            .snapshot
            .local_row(view.local_peer)
            .ok_or("Chat sender absent")?;
        if text.is_empty() || recipients.is_some_and(|r| r.mask(local) == 0) {
            return Ok(false);
        }
        if text.len() > network::chat::Packet::MAX_TEXT || text.contains(&0) {
            return Err("Chat text is too long".into());
        }
        match self.commands.try_send(Command::Chat(text, recipients)) {
            Ok(()) => Ok(true),
            // A full queue drops the line; with no echo the sender sees it was not sent.
            Err(TrySendError::Full(_)) => Ok(false),
            Err(TrySendError::Disconnected(_)) => Err("Chat worker stopped".into()),
        }
    }
    /// One interface tick of the chat display countdown (0x452110).
    pub fn tick_chat(&self) {
        if let Ok(mut chat) = self.chat.lock() {
            chat.ring.tick452110();
        }
    }
    pub fn action(&self, action: Action) -> Result<(), String> {
        self.commands
            .try_send(Command::Edit(action))
            .map_err(|_| "Lobby is busy; try again.".into())
    }
    pub fn cancellation(&self) -> Cancellation {
        Cancellation {
            owner: self.owner,
            attempt: self.attempt,
            cancel: self.cancel.clone(),
        }
    }
    pub fn validate_binding(&self, binding: network::SessionBinding) -> Result<(), String> {
        self.owner
            .validate_binding(binding)
            .map_err(|e| e.to_string())
    }
    /// Staging and all peer waits stay on this worker. UI only polls updates.
    pub fn stage(
        &self,
        stage: impl FnOnce() -> Result<network::Content, String> + Send + 'static,
    ) -> Result<(), String> {
        self.commands
            .try_send(Command::Stage(Box::new(stage)))
            .map_err(|_| "Startup worker is busy".into())
    }
    pub fn prepared(
        &self,
        content: std::result::Result<network::Content, String>,
    ) -> Result<(), String> {
        self.commands
            .try_send(Command::Prepared(content))
            .map_err(|_| "Startup worker is busy".into())
    }
    pub fn bind_world(&self, world: u64) -> Result<network::SessionBinding, String> {
        self.owner
            .bind_world(self.attempt, world)
            .map_err(|e| e.to_string())
    }
    pub fn validate(&self) -> Result<(), String> {
        self.owner
            .lock()
            .and_then(|s| {
                s.attempt(self.attempt)?;
                if matches!(s.phase, network::Phase::Offline | network::Phase::Stopped) {
                    return Err(network::Error::Phase);
                }
                Ok(())
            })
            .map_err(|e| e.to_string())
    }
    pub fn gameplay_owner(&self) -> &Owner {
        self.owner
    }
    pub fn poll_gameplay(&self) -> Option<network::commands::Ingress> {
        self.gameplay.try_recv().ok()
    }
    /// Send on the same ordered worker channel as all prior commits. `None`
    /// broadcasts from the host, or sends to the authority on a client.
    pub fn send_gameplay(
        &self,
        binding: network::SessionBinding,
        target: Option<Connection>,
        message: Message,
    ) -> Result<(), String> {
        self.validate_binding(binding)?;
        if !matches!(
            message,
            Message::CommandRequest(_)
                | Message::CommandCommit(_)
                | Message::CommandDeadline { .. }
                | Message::CommandRejected { .. }
        ) {
            return Err(network::Error::Unsupported.to_string());
        }
        self.commands
            .try_send(Command::Gameplay(binding, target, Box::new(message)))
            .map_err(|_| "Gameplay worker is busy".into())
    }
    pub fn poll_synchronization(&self) -> Option<network::synchronization::Ingress> {
        self.synchronization.try_recv().ok()
    }
    /// Return the real bounded transport enqueue result, not acceptance by the
    /// worker command channel. Owner::send only queues bytes under its lock;
    /// socket I/O and receive/loss maintenance remain on the worker thread.
    pub fn send_synchronization(
        &self,
        binding: network::SessionBinding,
        target: Option<u8>,
        message: Message,
    ) -> Result<bool, String> {
        if !matches!(message, Message::Barrier { .. } | Message::StateSample(_)) {
            return Err(network::Error::Tag.to_string());
        }
        let destinations = {
            let state = self.owner.lock().map_err(|e| e.to_string())?;
            state.binding(binding).map_err(|e| e.to_string())?;
            state
                .peers
                .iter()
                .enumerate()
                .filter_map(|(slot, peer)| {
                    let peer = peer.as_ref()?;
                    if target.is_some_and(|target| usize::from(target) != slot) {
                        return None;
                    }
                    Some(Connection {
                        attempt: self.attempt,
                        slot: slot as u8,
                        identity: peer.identity,
                    })
                })
                .collect::<Vec<_>>()
        };
        if target.is_some() && destinations.is_empty() {
            return Ok(false);
        }
        let mut sent = true;
        for connection in destinations {
            match self.owner.send(connection, message.clone()) {
                Ok(()) => (),
                Err(network::transport::Error::QueueFull) => sent = false,
                Err(error) => return Err(error.to_string()),
            }
        }
        Ok(sent)
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
struct Publications<'a> {
    lobby: &'a SyncSender<Update>,
    gameplay: &'a SyncSender<network::commands::Ingress>,
    synchronization: &'a SyncSender<network::synchronization::Ingress>,
    chat: &'a std::sync::Mutex<network::chat::Display>,
}
fn run(
    owner: &Owner,
    attempt: Attempt,
    open: Open,
    actions: Receiver<Command>,
    publications: Publications<'_>,
    cancel: &AtomicBool,
    network_timeout: Duration,
) -> Result<(), String> {
    let Publications {
        lobby: publish,
        gameplay,
        synchronization,
        chat,
    } = publications;
    if let Open::Probe(address) = &open {
        let (session, snapshot) =
            network::transport::probe(*address, network_timeout).map_err(|e| e.to_string())?;
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
            .connect(attempt, address, network_timeout)
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
    let mut starting: Option<Ticket> = None;
    let mut prepared = false;
    let mut ready = [false; SLOTS];
    let mut committed = false;
    let mut start_deadline = None;
    while !cancel.load(Ordering::Relaxed) {
        if start_deadline.is_some_and(|d| !committed && Instant::now() > d) {
            return Err("Timed out validating connected startup content.".into());
        }
        if !admitted && Instant::now() > deadline {
            return Err("Timed out waiting for session information or lobby admission.".into());
        }
        let mut changed = false;
        let (binding, constructing) = {
            let state = owner.lock().map_err(|e| e.to_string())?;
            state.attempt(attempt).map_err(|e| e.to_string())?;
            (
                state.binding,
                committed && state.phase == network::Phase::Starting,
            )
        };
        // Preserve unread TCP ingress while this process constructs its world.
        // No committed gameplay message may be consumed with an absent binding.
        let events = if constructing {
            owner.flush(attempt).map_err(|e| e.to_string())?;
            Vec::new()
        } else {
            owner.pump(attempt, binding).map_err(|e| e.to_string())?
        };
        for event in events {
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
                    Message::Chat(mut packet) => {
                        // Chat before admission, or from a peer without a roster row, is
                        // ignored. The host attributes a line to the connection it arrived
                        // on; a client only hears the host, which stamped the sender.
                        let Some(roster) = snapshot.as_ref().filter(|_| admitted) else {
                            continue;
                        };
                        let origin = if host {
                            connection.slot()
                        } else {
                            match packet.sender_peer() {
                                Some(peer) if peer != local_peer => peer,
                                _ => continue,
                            }
                        };
                        let Some(row) = roster.local_row(origin) else {
                            continue;
                        };
                        if host {
                            packet.sender = u32::from(origin) + 1;
                            // TCP is a star: relay to the selected rows, never back to
                            // the sender, whose own echo is already in its ring.
                            send_chat(owner, &peers, roster, &packet, Some(origin))?;
                        }
                        let local = roster
                            .local_row(local_peer)
                            .ok_or("Chat local row absent")?;
                        if packet.reaches(local) {
                            let mut display = chat.lock().map_err(|_| "Chat lock poisoned")?;
                            display.snapshot = Some(roster.clone());
                            display.local_peer = local_peer;
                            display
                                .append(row, &packet.text)
                                .map_err(|e| e.to_string())?;
                        }
                    }
                    message @ (Message::Barrier { .. } | Message::StateSample(_)) => {
                        synchronization
                            .try_send(network::synchronization::Ingress {
                                binding: binding.ok_or("Synchronization without a world")?,
                                connection,
                                message,
                            })
                            .map_err(|_| {
                                "Synchronization ingress queue is full or closed".to_string()
                            })?;
                    }
                    message @ (Message::CommandRequest(_)
                    | Message::CommandCommit(_)
                    | Message::CommandDeadline { .. }
                    | Message::CommandRejected { .. }) => {
                        gameplay
                            .try_send(network::commands::Ingress {
                                binding: binding.ok_or("Gameplay without a world")?,
                                connection,
                                message,
                            })
                            .map_err(|_| "Gameplay ingress queue is full or closed".to_string())?;
                    }
                    Message::LobbyRequest(request) if host => {
                        let roster = snapshot.as_mut().unwrap();
                        if starting.is_none() && roster.request(connection.slot(), request).is_ok()
                        {
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
                    Message::Start(start) if !host && admitted && starting.is_none() => {
                        let frozen = snapshot.clone().ok_or("Start without lobby")?;
                        if !frozen.start_enabled()
                            || start.content.level != frozen.settings.level
                            || start.content.parameter_bits != frozen.settings.parameter_bits
                            || start.content.difficulty != frozen.settings.difficulty
                        {
                            return Err(
                                "Connected startup parameters do not match the admitted lobby."
                                    .into(),
                            );
                        }
                        let ticket = Ticket {
                            view: View {
                                snapshot: frozen,
                                local_peer,
                                host,
                                address,
                            },
                            seed: start.seed,
                            expected: Some(start.content),
                        };
                        starting = Some(ticket.clone());
                        start_deadline = Some(Instant::now() + Duration::from_secs(60));
                        pending = Some(Update::Prepare(ticket));
                    }
                    Message::StartReady(content) if host && prepared && !committed => {
                        let ticket = starting.as_ref().ok_or("Unexpected startup readiness")?;
                        if ticket.view.snapshot.local_row(connection.slot()).is_none()
                            || ticket.expected != Some(content)
                        {
                            return Err("Connected startup content mismatch.".into());
                        }
                        ready[connection.slot() as usize] = true;
                    }
                    Message::StartCommit if !host && prepared && !committed => {
                        owner.begin_start(attempt).map_err(|e| e.to_string())?;
                        committed = true;
                        pending = Some(Update::Validated);
                    }
                    Message::StartFailure => {
                        return Err("A peer could not validate connected startup content.".into());
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
                    if starting.as_ref().is_some_and(|ticket| {
                        !host
                            || connection
                                .is_some_and(|c| ticket.view.snapshot.local_row(c.slot()).is_some())
                    }) {
                        return Err("Peer connection ended during connected startup.".into());
                    }
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
        for command in actions.try_iter().take(32) {
            let command = match command {
                Command::Stage(stage) => Command::Prepared(stage()),
                other => other,
            };
            let action = match command {
                Command::Chat(text, recipients) => {
                    let roster = snapshot
                        .as_ref()
                        .filter(|_| admitted)
                        .ok_or("Chat without admitted roster")?;
                    let local = roster.local_row(local_peer).ok_or("Chat sender absent")?;
                    let mask = recipients.map_or(0, |r| r.mask(local));
                    if text.is_empty() || recipients.is_some() && mask == 0 {
                        continue;
                    }
                    let packet = network::chat::Packet {
                        sender: if host { 1 } else { 0 },
                        recipients: mask,
                        text,
                    };
                    if host {
                        send_chat(owner, &peers, roster, &packet, None)?;
                    } else {
                        let authority = peers[0].ok_or("Chat authority absent")?;
                        match owner.send(authority, Message::Chat(packet.clone())) {
                            Ok(()) | Err(network::transport::Error::QueueFull) => (),
                            Err(error) => return Err(error.to_string()),
                        }
                    }
                    // The sender's one echo; no copy comes back from the host.
                    let mut display = chat.lock().map_err(|_| "Chat lock poisoned")?;
                    display.snapshot = Some(roster.clone());
                    display.local_peer = local_peer;
                    display
                        .append(local, &packet.text)
                        .map_err(|e| e.to_string())?;
                    continue;
                }
                Command::Gameplay(binding, target, message) => {
                    let message = *message;
                    owner.validate_binding(binding).map_err(|e| e.to_string())?;
                    let request = matches!(message, Message::CommandRequest(_));
                    if host == request {
                        return Err("Gameplay message has wrong local role".into());
                    }
                    let destinations: Vec<_> = if let Some(target) = target {
                        owner
                            .validate_connection(binding, target, false)
                            .map_err(|e| e.to_string())?;
                        vec![target]
                    } else if host {
                        starting
                            .as_ref()
                            .ok_or("Gameplay without Start")?
                            .view
                            .snapshot
                            .rows
                            .iter()
                            .filter_map(|r| r.peer)
                            .filter(|&p| p != 0)
                            .map(|p| peers[p as usize].ok_or("Admitted gameplay peer missing"))
                            .collect::<Result<Vec<_>, _>>()?
                    } else {
                        vec![peers[0].ok_or("Authority connection missing")?]
                    };
                    for connection in destinations {
                        owner
                            .send(connection, message.clone())
                            .map_err(|e| e.to_string())?;
                    }
                    continue;
                }
                Command::Prepared(result) => {
                    let content = match result {
                        Ok(content) => content,
                        Err(error) => {
                            for connection in peers.iter().flatten() {
                                let _ = owner.send(*connection, Message::StartFailure);
                            }
                            let _ = owner.pump(attempt, None);
                            return Err(error);
                        }
                    };
                    let ticket = starting.as_mut().ok_or("Content without admitted Start")?;
                    if prepared {
                        return Err("Duplicate startup content".into());
                    }
                    if ticket.expected.is_some_and(|expected| expected != content) {
                        if let Some(connection) = peers[0] {
                            let _ = owner.send(connection, Message::StartFailure);
                        }
                        let _ = owner.pump(attempt, None);
                        return Err("Connected startup version/content digest mismatch.".into());
                    }
                    ticket.expected = Some(content);
                    prepared = true;
                    if host {
                        for peer in ticket
                            .view
                            .snapshot
                            .rows
                            .iter()
                            .filter_map(|r| r.peer)
                            .filter(|&p| p != 0)
                        {
                            let connection =
                                peers[peer as usize].ok_or("Admitted peer connection missing")?;
                            // Publish the exact final revision before Start on this ordered stream.
                            owner
                                .send(connection, Message::Lobby(ticket.view.snapshot.clone()))
                                .map_err(|e| e.to_string())?;
                            owner
                                .send(
                                    connection,
                                    Message::Start(network::Start {
                                        content,
                                        seed: ticket.seed,
                                    }),
                                )
                                .map_err(|e| e.to_string())?;
                        }
                        ready[0] = true;
                    } else {
                        owner
                            .send(
                                peers[0].ok_or("Host connection missing")?,
                                Message::StartReady(content),
                            )
                            .map_err(|e| e.to_string())?;
                    }
                    continue;
                }
                Command::Edit(action) => action,
                Command::Stage(_) => unreachable!(),
            };
            if starting.is_some() {
                continue;
            }
            let Some(roster) = snapshot.as_mut().filter(|_| admitted) else {
                continue;
            };
            if host {
                if action == Action::Start {
                    if roster.start_enabled() {
                        let ticket = Ticket {
                            view: View {
                                snapshot: roster.clone(),
                                local_peer,
                                host,
                                address,
                            },
                            seed: SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .map_err(|e| e.to_string())?
                                .as_secs() as u32,
                            expected: None,
                        };
                        starting = Some(ticket.clone());
                        start_deadline = Some(Instant::now() + Duration::from_secs(60));
                        pending = Some(Update::Prepare(ticket));
                    } else {
                        pending = Some(Update::Notice("At least two populated rows must have a character, name and readiness.".into()));
                    }
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
        if changed && starting.is_none() {
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
        if host
            && prepared
            && !committed
            && starting.as_ref().is_some_and(|t| {
                t.view
                    .snapshot
                    .rows
                    .iter()
                    .filter_map(|r| r.peer)
                    .all(|p| ready[p as usize])
            })
        {
            for peer in starting
                .as_ref()
                .unwrap()
                .view
                .snapshot
                .rows
                .iter()
                .filter_map(|r| r.peer)
                .filter(|&p| p != 0)
            {
                owner
                    .send(
                        peers[peer as usize].ok_or("Admitted peer missing")?,
                        Message::StartCommit,
                    )
                    .map_err(|e| e.to_string())?;
            }
            owner.begin_start(attempt).map_err(|e| e.to_string())?;
            committed = true;
            pending = Some(Update::Validated);
        }
        if host && starting.is_none() {
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
        if admitted {
            // Chat lines take their name and colour from the current admitted roster.
            let mut display = chat.lock().map_err(|_| "Chat lock poisoned")?;
            if display.snapshot != snapshot {
                display.snapshot = snapshot.clone();
            }
            display.local_peer = local_peer;
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
pub(crate) mod tests;

/// Host relay: sends `packet` to the connected peer of every row it reaches, except the
/// host itself and `exclude` (the originating peer).
fn send_chat(
    owner: &Owner,
    peers: &[Option<Connection>; SLOTS],
    roster: &Snapshot,
    packet: &network::chat::Packet,
    exclude: Option<u8>,
) -> Result<(), String> {
    for (row, slot) in roster.rows.iter().enumerate() {
        let Some(peer) = slot.peer.filter(|&p| p != 0 && Some(p) != exclude) else {
            continue;
        };
        // A peer whose connection already ended leaves the roster on its failure event.
        let Some(connection) = peers[usize::from(peer)].filter(|_| packet.reaches(row)) else {
            continue;
        };
        match owner.send(connection, Message::Chat(packet.clone())) {
            Ok(()) | Err(network::transport::Error::QueueFull) => (),
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(())
}
