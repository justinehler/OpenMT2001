use super::*;
use crate::network::{Frame, Message, Transfer, codec};
use std::collections::VecDeque;

fn frame(sequence: u64) -> Vec<u8> {
    codec::encode(&Frame {
        session: [7; 16],
        sequence,
        message: Message::Transfer(Transfer {
            category: 0,
            owner: 1,
            amount: sequence as i32,
            due: 25,
        }),
    })
    .unwrap()
}

/// Scripted I/O makes short calls, WouldBlock and failures deterministic on
/// every platform, rather than depending on a particular TCP buffer size.
struct Reader {
    bytes: VecDeque<u8>,
    chunk: usize,
    end: io::ErrorKind,
}
impl Read for Reader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let count = out.len().min(self.chunk).min(self.bytes.len());
        if count == 0 {
            if self.end == io::ErrorKind::UnexpectedEof {
                return Ok(0);
            }
            return Err(self.end.into());
        }
        for byte in &mut out[..count] {
            *byte = self.bytes.pop_front().unwrap();
        }
        Ok(count)
    }
}
fn reader(bytes: Vec<u8>, chunk: usize, end: io::ErrorKind) -> Reader {
    Reader {
        bytes: bytes.into(),
        chunk,
        end,
    }
}

#[test]
fn byte_at_a_time_retains_header_and_body_across_would_block() {
    let expected = frame(0);
    let mut framing = Framing::default();
    for (index, byte) in expected.iter().enumerate() {
        let mut input = reader(vec![*byte], 1, io::ErrorKind::WouldBlock);
        let batch = framing.read(&mut input);
        assert_eq!(batch.terminal, None);
        if index + 1 == expected.len() {
            assert_eq!(batch.frames.as_slice(), std::slice::from_ref(&expected));
        } else {
            assert!(batch.frames.is_empty());
        }
    }
    assert_eq!(framing.filled, 0);
}

#[test]
fn coalesced_frames_precede_eof_read_error_and_truncated_tail() {
    let expected = vec![frame(0), frame(1), frame(2)];
    for end in [io::ErrorKind::UnexpectedEof, io::ErrorKind::ConnectionReset] {
        for partial in [0, 1, HEADER - 1, HEADER, frame(3).len() - 1] {
            let mut bytes = expected.concat();
            bytes.extend_from_slice(&frame(3)[..partial]);
            let mut input = reader(bytes, usize::MAX, end);
            let batch = Framing::default().read(&mut input);
            assert_eq!(batch.frames, expected);
            assert_eq!(
                batch.terminal,
                Some(if end == io::ErrorKind::ConnectionReset {
                    Error::Io(end)
                } else if partial == 0 {
                    Error::Closed
                } else {
                    Error::Truncated
                })
            );
        }
    }
}

#[test]
fn oversized_header_stops_without_allocating_its_body() {
    let mut header = frame(0)[..HEADER].to_vec();
    header[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
    let batch = Framing::default().read(&mut header.as_slice());
    assert!(batch.frames.is_empty());
    assert_eq!(
        batch.terminal,
        Some(Error::Protocol(crate::network::Error::Oversized))
    );
}

struct Writer {
    bytes: Vec<u8>,
    chunk: usize,
    calls_left: usize,
    end: io::ErrorKind,
}
impl Write for Writer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.calls_left == 0 {
            return Err(self.end.into());
        }
        self.calls_left -= 1;
        let count = bytes.len().min(self.chunk);
        self.bytes.extend_from_slice(&bytes[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn partial_writes_keep_offsets_and_queue_full_is_reported() {
    let mut framing = Framing::default();
    let expected: Vec<_> = (0..MAX_OUTPUT_FRAMES as u64).map(frame).collect();
    for bytes in &expected {
        framing.enqueue(bytes.clone()).unwrap();
    }
    assert_eq!(framing.enqueue(frame(16)), Err(Error::QueueFull));
    let reserved = framing.reserved;
    let mut writer = Writer {
        bytes: vec![],
        chunk: 1,
        calls_left: 3,
        end: io::ErrorKind::WouldBlock,
    };
    framing.flush(&mut writer).unwrap();
    assert_eq!(writer.bytes, expected[0][..3]);
    assert_eq!(framing.output.front().unwrap().offset, 3);
    assert_eq!(framing.reserved, reserved);
    // An error after a short write must not rewind the offset either.
    writer.calls_left = 2;
    writer.end = io::ErrorKind::BrokenPipe;
    assert_eq!(
        framing.flush(&mut writer),
        Err(Error::Io(io::ErrorKind::BrokenPipe))
    );
    assert_eq!(framing.output.front().unwrap().offset, 5);
    writer.chunk = usize::MAX;
    writer.calls_left = usize::MAX;
    framing.flush(&mut writer).unwrap();
    assert_eq!(writer.bytes, expected.concat());
    assert!(framing.output.is_empty());
    assert_eq!(framing.reserved, 0);
    framing.enqueue(frame(16)).unwrap();
}

#[test]
fn work_limits_include_short_and_interrupted_calls() {
    let mut framing = Framing::default();
    let bytes = frame(0);
    let mut input = reader(bytes.clone(), 1, io::ErrorKind::WouldBlock);
    assert!(framing.read(&mut input).frames.is_empty());
    assert_eq!(framing.filled, MAX_IO_CALLS);
    assert_eq!(
        framing.read(&mut input).frames.as_slice(),
        std::slice::from_ref(&bytes)
    );
    framing.enqueue(bytes).unwrap();
    let mut writer = Writer {
        bytes: vec![],
        chunk: 1,
        calls_left: usize::MAX,
        end: io::ErrorKind::WouldBlock,
    };
    framing.flush(&mut writer).unwrap();
    assert_eq!(writer.bytes.len(), MAX_IO_CALLS);

    struct Interrupted(usize);
    impl Read for Interrupted {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            self.0 += 1;
            Err(io::ErrorKind::Interrupted.into())
        }
    }
    impl Write for Interrupted {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            self.0 += 1;
            Err(io::ErrorKind::Interrupted.into())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut interrupted = Interrupted(0);
    assert!(framing.read(&mut interrupted).frames.is_empty());
    assert_eq!(interrupted.0, MAX_IO_CALLS);
    framing.flush(&mut interrupted).unwrap();
    assert_eq!(interrupted.0, 2 * MAX_IO_CALLS);
}

#[test]
fn frame_budget_defers_remaining_frames_without_losing_them() {
    // Zero-payload Hello frames take one read each, exercising the frame budget
    // independently of the syscall budget.
    let hello = codec::encode(&Frame {
        session: [0; 16],
        sequence: 0,
        message: Message::Hello,
    })
    .unwrap();
    let bytes = hello.repeat(MAX_INPUT_FRAMES + 2);
    let mut input = reader(bytes, usize::MAX, io::ErrorKind::UnexpectedEof);
    let mut framing = Framing::default();
    let first = framing.read(&mut input);
    assert_eq!(first.frames, vec![hello.clone(); MAX_INPUT_FRAMES]);
    assert_eq!(first.terminal, None);
    let second = framing.read(&mut input);
    assert_eq!(second.frames, vec![hello; 2]);
    assert_eq!(second.terminal, Some(Error::Closed));
}
