//! Bounded v1 codec, deliberately distinct from DirectPlay (send: 0x4e2410).
//! Fixed 36-byte header, all integers little-endian:
//! [0..4] "OMT1", [4..6] version u16, [6..8] tag u16,
//! [8..12] payload byte length u32, [12..28] session bytes, [28..36] sequence u64.
//! Tags: 1=roster, 2=start, 3=committed transfer, 4=Hello, 5=Welcome.
//! Hello has a zero session and no payload; Welcome has the host's session and
//! one assigned-slot byte (1 through 5). Both use sequence zero, outside the application
//! sequence. Payloads are field encodings,
//! never Rust/native struct copies. Sequences start at zero per adopted link.
use super::{Content, Error, Frame, Message, Result, Row, RowKind, SLOTS, Start, Transfer};

pub const HEADER: usize = 36;
pub const MAX_FRAME: usize = 512;

/// Per-connection session/sequence validation. Advance only after full decoding.
pub struct Decoder {
    session: [u8; 16],
    next: u64,
}
impl Decoder {
    pub fn new(session: [u8; 16]) -> Result<Self> {
        if session == [0; 16] {
            return Err(Error::Session);
        }
        Ok(Self { session, next: 0 })
    }
    pub fn decode(&mut self, bytes: &[u8]) -> Result<Frame> {
        let frame = decode(bytes)?;
        self.accept(&frame)?;
        Ok(frame)
    }
    pub(super) fn accept(&mut self, frame: &Frame) -> Result<()> {
        if matches!(frame.message, Message::Hello | Message::Welcome { .. }) {
            return Err(Error::Tag);
        }
        if frame.session != self.session {
            return Err(Error::Session);
        }
        if frame.sequence != self.next {
            return Err(Error::Sequence);
        }
        self.next = self.next.checked_add(1).ok_or(Error::Exhausted)?;
        Ok(())
    }
}

fn validate(message: &Message) -> Result<()> {
    match message {
        Message::Hello => {}
        Message::Welcome { slot } => {
            if !(1..SLOTS as u8).contains(slot) {
                return Err(Error::Peer);
            }
        }
        Message::Roster { content, rows } => {
            content.validate()?;
            let mut tokens = [false; 10];
            for row in rows {
                row.validate()?;
                if matches!(row.kind, RowKind::Human | RowKind::Ai) {
                    let token = row.token as usize;
                    if tokens[token] {
                        return Err(Error::Row);
                    }
                    tokens[token] = true;
                }
            }
        }
        Message::Start(start) => start.content.validate()?,
        Message::Transfer(transfer) => {
            if transfer.category >= 52 || !(1..=6).contains(&transfer.owner) {
                return Err(Error::Row);
            }
        }
    }
    Ok(())
}

fn content(out: &mut Vec<u8>, content: Content) {
    out.extend_from_slice(&content.level.to_le_bytes());
    out.extend_from_slice(&content.parameter_bits.to_le_bytes());
    out.push(content.difficulty);
    out.extend_from_slice(&content.fingerprint);
}
pub fn encode(frame: &Frame) -> Result<Vec<u8>> {
    envelope(frame)?;
    validate(&frame.message)?;
    let mut payload = Vec::new();
    let tag: u16 = match &frame.message {
        Message::Hello => 4,
        Message::Welcome { slot } => {
            payload.push(*slot);
            5
        }
        Message::Roster {
            content: setup,
            rows,
        } => {
            content(&mut payload, *setup);
            // Six rows in slot order: kind u8, character i8, ready u8,
            // name byte length u8, followed by that many non-NUL bytes.
            for row in rows {
                payload.push(match row.kind {
                    RowKind::Human => 0,
                    RowKind::Ai => 1,
                    RowKind::Open => 2,
                    RowKind::Closed => 3,
                });
                payload.extend_from_slice(&[
                    row.token as u8,
                    u8::from(row.ready),
                    row.name.len() as u8,
                ]);
                payload.extend_from_slice(&row.name);
            }
            1
        }
        Message::Start(start) => {
            // Content: level u32, parameter f32 bits u32, difficulty u8,
            // fingerprint [u8;32]. Start appends seed u32.
            content(&mut payload, start.content);
            payload.extend_from_slice(&start.seed.to_le_bytes());
            2
        }
        Message::Transfer(transfer) => {
            // category u8, owner u8, amount i32, Total due u32.
            payload.extend_from_slice(&[transfer.category, transfer.owner]);
            payload.extend_from_slice(&transfer.amount.to_le_bytes());
            payload.extend_from_slice(&transfer.due.to_le_bytes());
            3
        }
    };
    if payload.len() > MAX_FRAME - HEADER {
        return Err(Error::Oversized);
    }
    let mut out = Vec::with_capacity(HEADER + payload.len());
    out.extend_from_slice(b"OMT1");
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&tag.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&frame.session);
    out.extend_from_slice(&frame.sequence.to_le_bytes());
    out.extend_from_slice(&payload);
    Ok(out)
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N]> {
        let end = self.at.checked_add(N).ok_or(Error::Oversized)?;
        let bytes = self.bytes.get(self.at..end).ok_or(Error::Truncated)?;
        self.at = end;
        Ok(bytes.try_into().unwrap())
    }
    fn byte(&mut self) -> Result<u8> {
        Ok(self.take::<1>()?[0])
    }
    fn word(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take()?))
    }
    fn content(&mut self) -> Result<Content> {
        Ok(Content {
            level: self.word()?,
            parameter_bits: self.word()?,
            difficulty: self.byte()?,
            fingerprint: self.take()?,
        })
    }
    fn row(&mut self) -> Result<Row> {
        let kind = match self.byte()? {
            0 => RowKind::Human,
            1 => RowKind::Ai,
            2 => RowKind::Open,
            3 => RowKind::Closed,
            _ => return Err(Error::Row),
        };
        let token = self.byte()? as i8;
        let ready = match self.byte()? {
            0 => false,
            1 => true,
            _ => return Err(Error::Row),
        };
        let length = usize::from(self.byte()?);
        if length > 31 {
            return Err(Error::Row);
        }
        let end = self.at + length;
        let name = self
            .bytes
            .get(self.at..end)
            .ok_or(Error::Truncated)?
            .to_vec();
        self.at = end;
        Ok(Row {
            kind,
            token,
            ready,
            name,
        })
    }
}

/// Structural decoding only. Use `Decoder` or `Owner::receive` to reject replay
/// before any effects; bare frames carry no sender identity or authority.
pub fn decode(bytes: &[u8]) -> Result<Frame> {
    if bytes.len() > MAX_FRAME {
        return Err(Error::Oversized);
    }
    let mut header = Reader { bytes, at: 0 };
    if header.take::<4>()? != *b"OMT1" {
        return Err(Error::Magic);
    }
    if u16::from_le_bytes(header.take()?) != 1 {
        return Err(Error::Version);
    }
    let tag = u16::from_le_bytes(header.take()?);
    let length = header.word()? as usize;
    if length > MAX_FRAME - HEADER {
        return Err(Error::Oversized);
    }
    let session = header.take()?;
    let sequence = u64::from_le_bytes(header.take()?);
    if bytes.len() < HEADER + length {
        return Err(Error::Truncated);
    }
    if bytes.len() > HEADER + length {
        return Err(Error::TrailingBytes);
    }
    let mut reader = Reader {
        bytes: &bytes[HEADER..],
        at: 0,
    };
    let message = match tag {
        1 => {
            let content = reader.content()?;
            let mut rows = Vec::with_capacity(SLOTS);
            for _ in 0..SLOTS {
                rows.push(reader.row()?);
            }
            Message::Roster {
                content,
                rows: rows.try_into().map_err(|_| Error::Row)?,
            }
        }
        2 => Message::Start(Start {
            content: reader.content()?,
            seed: reader.word()?,
        }),
        3 => Message::Transfer(Transfer {
            category: reader.byte()?,
            owner: reader.byte()?,
            amount: reader.word()? as i32,
            due: reader.word()?,
        }),
        4 => Message::Hello,
        5 => Message::Welcome {
            slot: reader.byte()?,
        },
        _ => return Err(Error::Tag),
    };
    if reader.at != reader.bytes.len() {
        return Err(Error::TrailingBytes);
    }
    validate(&message)?;
    let frame = Frame {
        session,
        sequence,
        message,
    };
    envelope(&frame)?;
    Ok(frame)
}

fn envelope(frame: &Frame) -> Result<()> {
    if matches!(frame.message, Message::Hello) != (frame.session == [0; 16]) {
        return Err(Error::Session);
    }
    if matches!(frame.message, Message::Hello | Message::Welcome { .. }) && frame.sequence != 0 {
        return Err(Error::Sequence);
    }
    Ok(())
}
