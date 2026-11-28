//! Process multiplayer owner and modern OpenMT peer protocol, independent of Lua.
//! Direct-IP transport is pumped by the lobby worker through this owner.
//! Connected startup and command consumers follow in #30. Only explicit
//! listen/connect/probe calls open sockets; connected startup is still separate.
//! Ownership references: 0x4e1020, 0x4e2740, 0x451fe0; peer creation: 0x4e1360.
use std::sync::{
    Mutex, MutexGuard, OnceLock,
    atomic::{AtomicU64, Ordering},
};

pub mod codec;
pub mod lobby;
pub mod stream;
#[cfg(test)]
mod tests;
pub mod transport;

pub const SLOTS: usize = 6;
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Phase,
    StaleAttempt,
    World,
    Peer,
    Poisoned,
    Exhausted,
    Truncated,
    TrailingBytes,
    Oversized,
    Magic,
    Version,
    Tag,
    Session,
    Sequence,
    Row,
    Content,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "multiplayer: {self:?}")
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Offline,
    Opening,
    Lobby,
    Starting,
    Playing,
    Stopped,
}

/// Local capability, never a remote-provided identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attempt {
    owner: u64,
    number: u64,
}

/// Store on the actual Session. Even identical wire session IDs cannot make an
/// earlier attempt's binding valid for another owner, attempt or loaded world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionBinding {
    attempt: Attempt,
    world: u64,
}
impl SessionBinding {
    pub fn attempt(self) -> Attempt {
        self.attempt
    }
    pub fn world(self) -> u64 {
        self.world
    }
}

/// Minted by trusted connection adoption, never decoded from a payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Connection {
    attempt: Attempt,
    slot: u8,
    identity: u64,
}
impl Connection {
    pub fn slot(self) -> u8 {
        self.slot
    }
}

struct Peer {
    identity: u64,
    authority: bool,
    decoder: codec::Decoder,
}
struct State {
    phase: Phase,
    attempt: Attempt,
    session: Option<[u8; 16]>,
    binding: Option<SessionBinding>,
    peers: [Option<Peer>; SLOTS],
    next_connection: u64,
    transport: Option<transport::Transport>,
}
impl State {
    fn attempt(&self, attempt: Attempt) -> Result<()> {
        if self.attempt != attempt {
            return Err(Error::StaleAttempt);
        }
        Ok(())
    }
    fn phase(&self, phase: Phase) -> Result<()> {
        if self.phase != phase {
            return Err(Error::Phase);
        }
        Ok(())
    }
    fn binding(&self, binding: SessionBinding) -> Result<()> {
        self.attempt(binding.attempt)?;
        self.phase(Phase::Playing)?;
        if self.binding != Some(binding) {
            return Err(Error::World);
        }
        Ok(())
    }
    fn opened(&mut self, attempt: Attempt, session: [u8; 16]) -> Result<()> {
        self.attempt(attempt)?;
        self.phase(Phase::Opening)?;
        if session == [0; 16] {
            return Err(Error::Session);
        }
        self.session = Some(session);
        self.phase = Phase::Lobby;
        Ok(())
    }
    fn adopt(&mut self, attempt: Attempt, slot: u8, authority: bool) -> Result<Connection> {
        self.attempt(attempt)?;
        self.phase(Phase::Lobby)?;
        let index = usize::from(slot);
        if index >= SLOTS
            || self.peers[index].is_some()
            || (authority && self.peers.iter().flatten().any(|peer| peer.authority))
        {
            return Err(Error::Peer);
        }
        let decoder = codec::Decoder::new(self.session.ok_or(Error::Session)?)?;
        let identity = self.next_connection;
        self.next_connection = identity.checked_add(1).ok_or(Error::Exhausted)?;
        self.peers[index] = Some(Peer {
            identity,
            authority,
            decoder,
        });
        Ok(Connection {
            attempt,
            slot,
            identity,
        })
    }
    fn receive(
        &mut self,
        connection: Connection,
        binding: Option<SessionBinding>,
        bytes: &[u8],
    ) -> Result<Message> {
        let frame = codec::decode(bytes)?;
        self.attempt(connection.attempt)?;
        match &frame.message {
            Message::Roster { .. }
            | Message::Start(_)
            | Message::Lobby(_)
            | Message::LobbyRequest(_)
            | Message::LobbyDenied => self.phase(Phase::Lobby)?,
            Message::Transfer(_) => self.binding(binding.ok_or(Error::World)?)?,
            Message::Hello | Message::Probe | Message::Welcome { .. } => return Err(Error::Tag),
        }
        let peer = self.peers[usize::from(connection.slot)]
            .as_mut()
            .ok_or(Error::Peer)?;
        let request = matches!(frame.message, Message::LobbyRequest(_));
        if peer.identity != connection.identity || peer.authority == request {
            return Err(Error::Peer);
        }
        peer.decoder.accept(&frame)?;
        Ok(frame.message)
    }
}

/// The process singleton is obtained through `process()`. No guard or callback
/// escapes this API: adapters and simulation callbacks run after methods return.
/// Returned data is admission, not an applied simulation or transport effect.
pub struct Owner {
    state: Mutex<State>,
}
static OWNER_IDS: AtomicU64 = AtomicU64::new(1);
static PROCESS: OnceLock<Owner> = OnceLock::new();

pub fn process() -> &'static Owner {
    PROCESS.get_or_init(|| Owner::new().expect("multiplayer owner identity exhausted"))
}

impl Owner {
    fn new() -> Result<Self> {
        let owner = OWNER_IDS
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map_err(|_| Error::Exhausted)?;
        Ok(Self {
            state: Mutex::new(State {
                phase: Phase::Offline,
                attempt: Attempt { owner, number: 0 },
                session: None,
                binding: None,
                peers: std::array::from_fn(|_| None),
                next_connection: 0,
                transport: None,
            }),
        })
    }
    fn lock(&self) -> Result<MutexGuard<'_, State>> {
        self.state.lock().map_err(|_| Error::Poisoned)
    }
    pub fn phase(&self) -> Result<Phase> {
        Ok(self.lock()?.phase)
    }

    /// Opening is a request. Only the adapter's later `opened` completion can
    /// enter Lobby. There is no network switch that skips these transitions.
    pub fn begin(&self) -> Result<Attempt> {
        let mut state = self.lock()?;
        if !matches!(state.phase, Phase::Offline | Phase::Stopped) {
            return Err(Error::Phase);
        }
        let number = state
            .attempt
            .number
            .checked_add(1)
            .ok_or(Error::Exhausted)?;
        state.attempt.number = number;
        state.phase = Phase::Opening;
        state.session = None;
        state.binding = None;
        state.peers = std::array::from_fn(|_| None);
        state.next_connection = 0;
        state.transport = None;
        Ok(state.attempt)
    }
    pub fn opened(&self, attempt: Attempt, session: [u8; 16]) -> Result<()> {
        self.lock()?.opened(attempt, session)
    }
    /// Called by the trusted adapter with its assigned slot and role. A roster
    /// row is descriptive data and cannot modify these adopted identities.
    pub fn adopt(&self, attempt: Attempt, slot: u8, authority: bool) -> Result<Connection> {
        self.lock()?.adopt(attempt, slot, authority)
    }
    /// Coordinator calls after setup validation. This does not construct a world.
    pub fn begin_start(&self, attempt: Attempt) -> Result<()> {
        let mut state = self.lock()?;
        state.attempt(attempt)?;
        state.phase(Phase::Lobby)?;
        state.phase = Phase::Starting;
        Ok(())
    }
    /// Bind the loader's nonzero identity of the completed actual Session.
    /// Startup/content qualification belongs to the connected-startup slice.
    pub fn bind_world(&self, attempt: Attempt, world: u64) -> Result<SessionBinding> {
        let mut state = self.lock()?;
        state.attempt(attempt)?;
        state.phase(Phase::Starting)?;
        if world == 0 {
            return Err(Error::World);
        }
        let binding = SessionBinding { attempt, world };
        state.binding = Some(binding);
        state.phase = Phase::Playing;
        Ok(binding)
    }
    pub fn validate_binding(&self, binding: SessionBinding) -> Result<()> {
        self.lock()?.binding(binding)
    }
    /// Retire only the captured attempt. The adapter must close resources for
    /// that same attempt after return, never whichever connection is current.
    pub fn retire(&self, attempt: Attempt) -> Result<()> {
        let mut state = self.lock()?;
        state.attempt(attempt)?;
        if matches!(state.phase, Phase::Offline | Phase::Stopped) {
            return Err(Error::Phase);
        }
        state.phase = Phase::Stopped;
        state.binding = None;
        state.session = None;
        state.peers = std::array::from_fn(|_| None);
        state.transport = None;
        Ok(())
    }
    /// Admit authority commits or client lobby requests from adopted connections. Rejected input
    /// changes neither sequence nor lifecycle. No simulation callback runs here.
    /// A deferred consumer must revalidate its Session binding before applying.
    pub fn receive(
        &self,
        connection: Connection,
        binding: Option<SessionBinding>,
        bytes: &[u8],
    ) -> Result<Message> {
        self.lock()?.receive(connection, binding, bytes)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Content {
    pub level: u32,
    pub parameter_bits: u32,
    pub difficulty: u8,
    /// Identity of locally verified scripts/map/parameters, not a file path.
    pub fingerprint: [u8; 32],
}
impl Content {
    fn validate(self) -> Result<()> {
        if self.level == 0
            || self.difficulty > 2
            || self.fingerprint == [0; 32]
            || !f32::from_bits(self.parameter_bits).is_finite()
        {
            return Err(Error::Content);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind {
    Human,
    Ai,
    Open,
    Closed,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub kind: RowKind,
    pub token: i8,
    pub ready: bool,
    /// At most 31 non-NUL bytes; preserves the game's localized byte strings.
    pub name: Vec<u8>,
}
impl Row {
    fn validate(&self) -> Result<()> {
        if self.name.len() > 31 || self.name.contains(&0) {
            return Err(Error::Row);
        }
        match self.kind {
            RowKind::Human | RowKind::Ai => {
                if !(0..10).contains(&self.token) || self.name.is_empty() {
                    return Err(Error::Row);
                }
            }
            RowKind::Open | RowKind::Closed => {
                if self.token != -1 || self.ready {
                    return Err(Error::Row);
                }
            }
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Start {
    pub content: Content,
    pub seed: u32,
}
/// Authority-committed property transfer, not a client request. `due` is the
/// already stamped Total deadline; consumers must not add the delay again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transfer {
    pub category: u8,
    pub owner: u8,
    pub amount: i32,
    pub due: u32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    /// Admission only: zero session, zero sequence, no payload or chosen role.
    Hello,
    /// Read-only admission: returns metadata without allocating a player peer.
    Probe,
    /// Admission only: the host's nonzero session and assigned client slot.
    /// The authority is always slot zero. Application sequences restart at zero.
    Welcome {
        slot: u8,
    },
    Roster {
        content: Content,
        rows: [Row; SLOTS],
    },
    /// Host metadata and six-row revision; distinct from verified startup content.
    Lobby(lobby::Snapshot),
    LobbyDenied,
    /// Only the adopted client connection identifies the requester.
    LobbyRequest(lobby::Request),
    Start(Start),
    Transfer(Transfer),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub session: [u8; 16],
    pub sequence: u64,
    pub message: Message,
}
