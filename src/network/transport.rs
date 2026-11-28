//! Private direct-IP TCP between OpenMT peers, not retail DirectPlay or discovery.
//! Host/Connect/enumerate references: 0x4e13c0, 0x4e2240, 0x4e1e70.
//! #33 wires this owner API into the lobby. Call `Owner::pump` explicitly there
//! and later from the 5 Hz callback (0x450410); this module starts no thread.
//!
//! At most five peer streams (including unfinished handshakes) and five accepts per
//! pump. Each link has the framing limits in `stream`; handshakes expire after
//! ten seconds when pumped. Slot zero is the host; slots 1 through 5 are assigned.
//! Hello/Welcome are admission frames, outside the application sequence. A
//! successful TCP connect alone is never a `Connection`. No authentication or
//! encryption is provided: use only on a trusted private network.
use super::{
    Attempt, Connection, Frame, Message, Owner, Phase, SLOTS, SessionBinding, State, codec,
    stream::Stream,
};
use std::{
    io,
    net::{SocketAddr, TcpListener, TcpStream},
    time::{Duration, Instant},
};

pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Protocol(super::Error),
    Io(io::ErrorKind),
    Closed,
    Truncated,
    QueueFull,
    Handshake,
    Timeout,
    Unavailable,
}
impl From<super::Error> for Error {
    fn from(error: super::Error) -> Self {
        Self::Protocol(error)
    }
}
impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error.kind())
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "multiplayer TCP: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Returned in receive order, with all admitted messages before link failure
/// (0x4e2740). A later consumer must still revalidate its world/attempt binding.
#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    Connected {
        connection: Connection,
        local_slot: u8,
    },
    Message {
        connection: Connection,
        message: Box<Message>,
    },
    Failed {
        connection: Option<Connection>,
        error: Error,
    },
}

#[derive(Clone, Copy)]
enum Admission {
    Hello,
    WelcomePending,
    HelloPending,
    Welcome,
    Connected(Connection),
}
struct Link {
    stream: Stream,
    admission: Admission,
    deadline: Instant,
    next_out: u64,
}
impl Link {
    fn new(socket: TcpStream, admission: Admission) -> Result<Self, Error> {
        Ok(Self {
            stream: Stream::new(socket)?,
            admission,
            deadline: Instant::now() + HANDSHAKE_TIMEOUT,
            next_out: 0,
        })
    }
    fn connection(&self) -> Option<Connection> {
        match self.admission {
            Admission::Connected(connection) => Some(connection),
            _ => None,
        }
    }
}

pub(super) struct Transport {
    attempt: Attempt,
    listener: Option<TcpListener>,
    links: [Option<Link>; SLOTS],
}
impl Transport {
    fn accept(&mut self, state: &State, events: &mut Vec<Event>) {
        if state.phase != Phase::Lobby {
            return;
        }
        let Some(listener) = &self.listener else {
            return;
        };
        for _ in 1..SLOTS {
            let Some(slot) =
                (1..SLOTS).find(|&slot| self.links[slot].is_none() && state.peers[slot].is_none())
            else {
                break;
            };
            match listener.accept() {
                Ok((socket, _)) => match Link::new(socket, Admission::Hello) {
                    Ok(link) => self.links[slot] = Some(link),
                    Err(error) => events.push(Event::Failed {
                        connection: None,
                        error,
                    }),
                },
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => {
                    events.push(Event::Failed {
                        connection: None,
                        error: error.into(),
                    });
                    break;
                }
            }
        }
    }
    fn pump(&mut self, state: &mut State, binding: Option<SessionBinding>) -> Vec<Event> {
        let mut events = Vec::new();
        self.accept(state, &mut events);
        for slot in 0..SLOTS {
            let Some(mut link) = self.links[slot].take() else {
                continue;
            };
            if link.connection().is_none() && Instant::now() >= link.deadline {
                events.push(Event::Failed {
                    connection: None,
                    error: Error::Timeout,
                });
                continue;
            }
            let batch = link.stream.read();
            let mut failure = if matches!(link.admission, Admission::WelcomePending) {
                state.phase(Phase::Lobby).err().map(Error::from)
            } else {
                None
            };
            for bytes in batch.frames {
                if let Err(error) =
                    self.receive(state, &mut link, slot as u8, binding, &bytes, &mut events)
                {
                    failure = Some(error);
                    break;
                }
            }
            // Received complete frames are admitted before EOF/read/write errors.
            failure = failure.or(batch.terminal);
            if failure.is_none() {
                failure = link.stream.flush().err();
            }
            if failure.is_none()
                && matches!(link.admission, Admission::HelloPending)
                && link.stream.drained()
            {
                link.admission = Admission::Welcome;
            }
            if failure.is_none()
                && matches!(link.admission, Admission::WelcomePending)
                && link.stream.drained()
            {
                match state.adopt(self.attempt, slot as u8, false) {
                    Ok(connection) => {
                        link.admission = Admission::Connected(connection);
                        events.push(Event::Connected {
                            connection,
                            local_slot: 0,
                        });
                    }
                    Err(error) => failure = Some(error.into()),
                }
            }
            if let Some(error) = failure {
                let connection = link.connection();
                if connection.is_some() {
                    state.peers[slot] = None;
                }
                events.push(Event::Failed { connection, error });
                // Dropping only this captured link cannot close a later attempt.
            } else {
                self.links[slot] = Some(link);
            }
        }
        events
    }
    fn receive(
        &self,
        state: &mut State,
        link: &mut Link,
        slot: u8,
        binding: Option<SessionBinding>,
        bytes: &[u8],
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        match link.admission {
            Admission::Hello => {
                state.phase(Phase::Lobby)?;
                let hello = codec::decode(bytes)?;
                if hello.message != Message::Hello {
                    return Err(Error::Handshake);
                }
                link.stream.enqueue(codec::encode(&Frame {
                    session: state.session.ok_or(super::Error::Session)?,
                    sequence: 0,
                    message: Message::Welcome { slot },
                })?)?;
                link.admission = Admission::WelcomePending;
            }
            Admission::Welcome => {
                let welcome = codec::decode(bytes)?;
                let Message::Welcome { slot: local_slot } = welcome.message else {
                    return Err(Error::Handshake);
                };
                // The host's session and assignment are authoritative. The client
                // never supplies either a local slot or an authority flag.
                state.opened(self.attempt, welcome.session)?;
                let connection = state.adopt(self.attempt, 0, true)?;
                link.admission = Admission::Connected(connection);
                events.push(Event::Connected {
                    connection,
                    local_slot,
                });
            }
            Admission::Connected(connection) => {
                let message = state.receive(connection, binding, bytes)?;
                events.push(Event::Message {
                    connection,
                    message: Box::new(message),
                });
            }
            Admission::WelcomePending | Admission::HelloPending => return Err(Error::Handshake),
        }
        Ok(())
    }
    fn send(
        &mut self,
        connection: Connection,
        session: [u8; 16],
        message: Message,
    ) -> Result<(), Error> {
        if matches!(message, Message::Hello | Message::Welcome { .. }) {
            return Err(Error::Handshake);
        }
        let link = self
            .links
            .get_mut(usize::from(connection.slot))
            .and_then(Option::as_mut)
            .ok_or(super::Error::Peer)?;
        if link.connection() != Some(connection) {
            return Err(super::Error::Peer.into());
        }
        let next = link
            .next_out
            .checked_add(1)
            .ok_or(super::Error::Exhausted)?;
        let bytes = codec::encode(&Frame {
            session,
            sequence: link.next_out,
            message,
        })?;
        link.stream.enqueue(bytes)?;
        link.next_out = next;
        Ok(())
    }
}

impl Owner {
    /// Bind and enter the host lobby only after the non-blocking listener exists.
    /// A nonzero session identity is supplied by the trusted local coordinator.
    /// Port zero is supported; return the actual address for direct-IP joining.
    pub fn listen(
        &self,
        attempt: Attempt,
        address: SocketAddr,
        session: [u8; 16],
    ) -> Result<SocketAddr, Error> {
        {
            let state = self.lock()?;
            state.attempt(attempt)?;
            state.phase(Phase::Opening)?;
            if state.transport.is_some() {
                return Err(super::Error::Phase.into());
            }
            if session == [0; 16] {
                return Err(super::Error::Session.into());
            }
        }
        let listener = TcpListener::bind(address)?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let mut state = self.lock()?;
        state.attempt(attempt)?;
        if state.transport.is_some() {
            return Err(super::Error::Phase.into());
        }
        state.opened(attempt, session)?;
        state.transport = Some(Transport {
            attempt,
            listener: Some(listener),
            links: std::array::from_fn(|_| None),
        });
        Ok(address)
    }
    /// std::net has no non-blocking connect constructor. Only this setup call
    /// waits, bounded by `timeout`; all subsequent I/O is non-blocking. No DNS is
    /// performed. Successful return means TCP setup, not handshake admission.
    pub fn connect(
        &self,
        attempt: Attempt,
        address: SocketAddr,
        timeout: Duration,
    ) -> Result<(), Error> {
        {
            let state = self.lock()?;
            state.attempt(attempt)?;
            state.phase(Phase::Opening)?;
            if state.transport.is_some() {
                return Err(super::Error::Phase.into());
            }
        }
        let socket = TcpStream::connect_timeout(&address, timeout)?;
        let mut link = Link::new(socket, Admission::HelloPending)?;
        link.stream.enqueue(codec::encode(&Frame {
            session: [0; 16],
            sequence: 0,
            message: Message::Hello,
        })?)?;
        self.install_client(attempt, link)
    }
    fn install_client(&self, attempt: Attempt, link: Link) -> Result<(), Error> {
        let mut state = self.lock()?;
        state.attempt(attempt)?;
        state.phase(Phase::Opening)?;
        if state.transport.is_some() {
            return Err(super::Error::Phase.into());
        }
        let mut links = std::array::from_fn(|_| None);
        links[0] = Some(link);
        state.transport = Some(Transport {
            attempt,
            listener: None,
            links,
        });
        Ok(())
    }
    /// The process owner is the sole pumper. Its lock serializes lifecycle,
    /// adoption and bounded I/O; no callbacks run while it is held. Return values
    /// do not mutate a world. Old completions, sends, pumps and retirement are
    /// rejected before touching the replacement attempt's sockets or peers.
    pub fn pump(
        &self,
        attempt: Attempt,
        binding: Option<SessionBinding>,
    ) -> Result<Vec<Event>, Error> {
        let mut state = self.lock()?;
        state.attempt(attempt)?;
        let mut transport = state.transport.take().ok_or(Error::Unavailable)?;
        let events = transport.pump(&mut state, binding);
        state.transport = Some(transport);
        Ok(events)
    }
    /// Queue one application frame for an admitted link. A full queue is an
    /// error; retry only after pumping. No sequence is consumed on rejection.
    pub fn send(&self, connection: Connection, message: Message) -> Result<(), Error> {
        let mut state = self.lock()?;
        state.attempt(connection.attempt)?;
        let session = state.session.ok_or(super::Error::Session)?;
        state
            .transport
            .as_mut()
            .ok_or(Error::Unavailable)?
            .send(connection, session, message)
    }
}

#[cfg(test)]
mod tests;
