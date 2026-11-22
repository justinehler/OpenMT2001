//! Original0044e230 train locator motion over a 60-byte route record.

use crate::player_manager::{put, word};

pub const RECORD_SIZE: usize = 60;

#[derive(Clone, Copy, Debug)]
pub struct Constants {
    pub speed_scale: f32,
    pub step: f32,
    pub zero: f32,
    pub max_distance: f32,
    pub origin: [f32; 3],
}
impl Constants {
    pub fn from_original(image: &crate::pe::PeImage<'_>, camera: [f32; 3]) -> Result<Self, String> {
        Ok(Self {
            speed_scale: image.f32_at(0x525248)?,
            step: image.f32_at(0x525b74)?,
            zero: image.f32_at(0x525040)?,
            max_distance: image.f32_at(0x525028)?,
            origin: camera,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioCall {
    pub kind: u32,
    pub record: usize,
    pub arguments: [u32; 3],
}

pub type AudioCallback<'a> =
    dyn FnMut(AudioCall, &mut [u8; RECORD_SIZE]) -> Result<(), String> + 'a;
pub struct Callbacks<'a> {
    pub audio: &'a mut AudioCallback<'a>,
}

fn float(record: &[u8; RECORD_SIZE], offset: usize) -> f32 {
    f32::from_bits(word(record, offset))
}

fn store_float(record: &mut [u8; RECORD_SIZE], offset: usize, value: f32) {
    put(record, offset, value.to_bits());
}

fn distance(record: &[u8; RECORD_SIZE], origin: [f32; 3]) -> Result<f32, String> {
    super::proximity::distance(
        [
            float(record, 0x18),
            float(record, 0x1c),
            -float(record, 0x20),
        ],
        origin,
    )
    .map(|v| v.to_f32())
}

fn audio(
    record_index: usize,
    record: &mut [u8; RECORD_SIZE],
    kind: u32,
    origin: [f32; 3],
    callbacks: &mut Callbacks<'_>,
) -> Result<(), String> {
    let args = [kind, distance(record, origin)?.to_bits(), 0];
    (callbacks.audio)(
        AudioCall {
            kind,
            record: record_index,
            arguments: args,
        },
        record,
    )
}

/// Execute one original0044e230 route record. Unknown state values return
/// without mutation, matching the native jump-table default branch.
pub fn advance(
    record_index: usize,
    record: &mut [u8; RECORD_SIZE],
    constants: Constants,
    callbacks: &mut Callbacks<'_>,
) -> Result<(), String> {
    use crate::x87::Sum;
    use std::cmp::Ordering;
    match word(record, 0) {
        1 => {
            let mut scaled = Sum::from_f32(float(record, 0x30))?;
            scaled.mul_f32(constants.speed_scale)?;
            if scaled.compare(Sum::from_f32(float(record, 0x38))?) == Ordering::Greater {
                let mut speed = Sum::from_f32(float(record, 0x30))?;
                speed.add(Sum::from_f32(constants.step)?.negated());
                store_float(record, 0x30, speed.to_f32());
                if speed.compare(Sum::from_f32(constants.zero)?) == Ordering::Less {
                    put(record, 0, 2);
                    put(record, 0x30, 0);
                }
            }
            let speed = Sum::from_f32(float(record, 0x30))?;
            let mut remaining = Sum::from_f32(float(record, 0x38))?;
            remaining.add(speed.negated());
            store_float(record, 0x38, remaining.to_f32());
            for (offset, velocity) in [(0x18, 0x24), (0x1c, 0x28), (0x20, 0x2c)] {
                let mut value = Sum::from_f32(float(record, velocity))?;
                value.mul(speed);
                value.add_f32(float(record, offset))?;
                store_float(record, offset, value.to_f32());
            }
        }
        2 => {
            audio(record_index, record, 11, constants.origin, callbacks)?;
            put(record, 0x14, 250);
            put(record, 0, 3);
        }
        3 => {
            let countdown = word(record, 0x14).wrapping_sub(1);
            put(record, 0x14, countdown);
            if (countdown as i32) > 0 {
                return Ok(());
            }
            put(record, 0, 4);
            audio(record_index, record, 12, constants.origin, callbacks)?;
        }
        4 => {
            // FST preserves the unrounded PC64 speed on the native stack.
            let mut speed = Sum::from_f32(float(record, 0x30))?;
            speed.add_f32(constants.step)?;
            store_float(record, 0x30, speed.to_f32());
            let mut remaining = speed;
            remaining.add_f32(float(record, 0x38))?;
            store_float(record, 0x38, remaining.to_f32());
            for (offset, velocity) in [(0x18, 0x24), (0x1c, 0x28), (0x20, 0x2c)] {
                let mut change = speed;
                change.mul_f32(float(record, velocity))?;
                let mut value = Sum::from_f32(float(record, offset))?;
                value.add(change.negated());
                store_float(record, offset, value.to_f32());
            }
            if speed.compare(Sum::from_f32(constants.max_distance)?) == Ordering::Greater {
                store_float(record, 0x30, 2.0);
            }
            if float(record, 0x38) < float(record, 0x34) {
                return Ok(());
            }
            audio(record_index, record, 9, constants.origin, callbacks)?;
            put(record, 0, 0);
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Reader<'a>(&'a [u8]);
    impl<'a> Reader<'a> {
        fn take(&mut self, length: usize) -> &'a [u8] {
            let (head, tail) = self.0.split_at(length);
            self.0 = tail;
            head
        }
        fn word(&mut self) -> u32 {
            u32::from_le_bytes(self.take(4).try_into().unwrap())
        }
        fn blob(&mut self) -> Vec<u8> {
            let length = self.word() as usize;
            self.take(length).to_vec()
        }
    }

    #[test]
    #[ignore = "requires original0044e230 train motion observations"]
    fn matches_native_train_motion() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_TRAIN_MOTION").unwrap()).unwrap();
        let mut reader = Reader(&bytes);
        assert_eq!(reader.take(8), b"MTTRN44\0");
        assert_eq!(reader.word(), 1);
        let cases = reader.word();
        let constants = Constants {
            speed_scale: 50.0,
            step: 0.06,
            zero: 0.0,
            max_distance: 2.0,
            origin: [0.0; 3],
        };
        for case in 0..cases {
            let name = String::from_utf8(reader.blob()).unwrap();
            let index = reader.word() as usize;
            let value_count = reader.word() as usize;
            for _ in 0..value_count {
                let _offset = reader.word();
                let _value = reader.word();
            }
            let mutation_count = reader.word() as usize;
            let mutations: Vec<(usize, u32)> = (0..mutation_count)
                .map(|_| (reader.word() as usize, reader.word()))
                .collect();
            let before = reader.blob();
            let expected = reader.blob();
            let return_reached = reader.word() != 0;
            let fault = reader.word() != 0;
            let call_count = reader.word() as usize;
            let expected_calls: Vec<(u32, [u32; 3])> = (0..call_count)
                .map(|_| {
                    let address = reader.word();
                    let _ecx = reader.word();
                    let args = [reader.word(), reader.word(), reader.word()];
                    let _reserved = reader.word();
                    (address, args)
                })
                .collect();
            let mut record: [u8; RECORD_SIZE] = before.try_into().unwrap();
            let mut actual_calls = Vec::new();
            let mut audio = |call: AudioCall, record: &mut [u8; RECORD_SIZE]| {
                for (offset, value) in &mutations {
                    put(record, *offset, *value);
                }
                actual_calls.push((0x004aefc0, call.arguments));
                Ok(())
            };
            let mut callbacks = Callbacks { audio: &mut audio };
            if !float(&record, 0x30).is_finite() && word(&record, 0) == 1 {
                assert!(
                    advance(index, &mut record, constants, &mut callbacks).is_err(),
                    "nonfinite native input must remain an explicit unsupported boundary"
                );
                continue;
            }
            advance(index, &mut record, constants, &mut callbacks).unwrap();
            assert!(
                return_reached && !fault,
                "native case {case} {name} did not return"
            );
            assert_eq!(actual_calls, expected_calls, "audio case {case} {name}");
            assert_eq!(
                record.as_slice(),
                expected.as_slice(),
                "record case {case} {name}"
            );
        }
        assert!(reader.0.is_empty());
    }
}
