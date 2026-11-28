//! Bounded non-blocking OMT1 framing, used only by the owner's TCP transport.
//! #33 connects the explicit pump to the lobby; the same pump will serve the
//! later 5 Hz callback (0x450410), with no reader thread or simulation loop.
use super::codec::{HEADER, MAX_FRAME};
use super::transport::Error;
use std::{
    collections::VecDeque,
    io::{self, Read, Write},
    net::TcpStream,
};

/// Per link: at most 16 queued frames, occupying at most 8 KiB (including sent
/// prefixes until their frame completes). Queue rejection never consumes a sequence.
pub const MAX_OUTPUT_FRAMES: usize = 16;
pub const MAX_OUTPUT_BYTES: usize = MAX_OUTPUT_FRAMES * MAX_FRAME;
/// Each pump makes at most 32 reads and 32 writes per link, and returns at most
/// 16 frames. Interrupted calls count too. A partial input uses at most 512 bytes.
pub const MAX_IO_CALLS: usize = 32;
pub const MAX_INPUT_FRAMES: usize = 16;

struct Pending {
    bytes: Vec<u8>,
    offset: usize,
}
struct Framing {
    input: [u8; MAX_FRAME],
    filled: usize,
    target: usize,
    output: VecDeque<Pending>,
    reserved: usize,
}
impl Default for Framing {
    fn default() -> Self {
        Self {
            input: [0; MAX_FRAME],
            filled: 0,
            target: HEADER,
            output: VecDeque::new(),
            reserved: 0,
        }
    }
}
#[derive(Default)]
pub(super) struct ReadBatch {
    pub frames: Vec<Vec<u8>>,
    pub terminal: Option<Error>,
}
impl Framing {
    fn enqueue(&mut self, bytes: Vec<u8>) -> Result<(), Error> {
        if !(HEADER..=MAX_FRAME).contains(&bytes.len()) {
            return Err(Error::Protocol(super::Error::Oversized));
        }
        if self.output.len() == MAX_OUTPUT_FRAMES || self.reserved + bytes.len() > MAX_OUTPUT_BYTES
        {
            return Err(Error::QueueFull);
        }
        self.reserved += bytes.len();
        self.output.push_back(Pending { bytes, offset: 0 });
        Ok(())
    }
    fn read(&mut self, reader: &mut impl Read) -> ReadBatch {
        let mut batch = ReadBatch::default();
        for _ in 0..MAX_IO_CALLS {
            match reader.read(&mut self.input[self.filled..self.target]) {
                Ok(0) => {
                    batch.terminal = Some(if self.filled == 0 {
                        Error::Closed
                    } else {
                        Error::Truncated
                    });
                    break;
                }
                Ok(count) => {
                    self.filled += count;
                    if self.filled == HEADER && self.target == HEADER {
                        let length =
                            u32::from_le_bytes(self.input[8..12].try_into().unwrap()) as usize;
                        if length > MAX_FRAME - HEADER {
                            batch.terminal = Some(Error::Protocol(super::Error::Oversized));
                            break;
                        }
                        self.target = HEADER + length;
                    }
                    if self.filled == self.target {
                        batch.frames.push(self.input[..self.target].to_vec());
                        self.filled = 0;
                        self.target = HEADER;
                        if batch.frames.len() == MAX_INPUT_FRAMES {
                            break;
                        }
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => {
                    batch.terminal = Some(Error::Io(error.kind()));
                    break;
                }
            }
        }
        batch
    }
    fn flush(&mut self, writer: &mut impl Write) -> Result<(), Error> {
        for _ in 0..MAX_IO_CALLS {
            let Some(front) = self.output.front_mut() else {
                break;
            };
            match writer.write(&front.bytes[front.offset..]) {
                Ok(0) => return Err(Error::Io(io::ErrorKind::WriteZero)),
                Ok(count) => {
                    front.offset += count;
                    if front.offset == front.bytes.len() {
                        self.reserved -= front.bytes.len();
                        self.output.pop_front();
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(Error::Io(error.kind())),
            }
        }
        Ok(())
    }
}

pub(super) struct Stream {
    socket: TcpStream,
    framing: Framing,
}
impl Stream {
    pub fn new(socket: TcpStream) -> Result<Self, Error> {
        socket.set_nonblocking(true)?;
        socket.set_nodelay(true)?;
        Ok(Self {
            socket,
            framing: Framing::default(),
        })
    }
    pub fn enqueue(&mut self, bytes: Vec<u8>) -> Result<(), Error> {
        self.framing.enqueue(bytes)
    }
    pub fn read(&mut self) -> ReadBatch {
        self.framing.read(&mut self.socket)
    }
    pub fn flush(&mut self) -> Result<(), Error> {
        self.framing.flush(&mut self.socket)
    }
    pub fn drained(&self) -> bool {
        self.framing.output.is_empty()
    }
}

#[cfg(test)]
mod tests;
