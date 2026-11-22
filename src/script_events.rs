//! Script-manager countdown list (004d5b40), distinct from the packet scheduler.
//! Lua references belong to the host VM; resolving, invoking and releasing them
//! stays explicit so callbacks can append events to this same live list.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    pub reference: u32,
    pub remaining: u32,
    pub argument: i32,
}

struct Node {
    event: Event,
    next: Option<usize>,
    previous: Option<usize>,
}

#[derive(Default)]
pub struct Events {
    nodes: Vec<Option<Node>>,
    free: Vec<usize>,
    head: Option<usize>,
    tail: Option<usize>,
    ticking: bool,
}

pub trait Host {
    /// None means the original weak Lua reference could not be resolved.
    /// A successful call supplies its first result through Lua tonumber.
    fn invoke(&mut self, event: Event, events: &mut Events) -> Result<Option<f64>, String>;
    fn release(&mut self, reference: u32) -> Result<(), String>;
}

impl Events {
    /// Append an already admitted reference (004d5c30). Zero-delay synchronous
    /// admission and EVENTINITIALISE belong to the script trigger binding.
    pub fn append(&mut self, event: Event) {
        let index = self.free.pop().unwrap_or_else(|| {
            self.nodes.push(None);
            self.nodes.len() - 1
        });
        self.nodes[index] = Some(Node {
            event,
            next: None,
            previous: self.tail,
        });
        if let Some(tail) = self.tail {
            self.nodes[tail].as_mut().unwrap().next = Some(index);
        } else {
            self.head = Some(index);
        }
        self.tail = Some(index);
    }

    pub fn pending(&self) -> Vec<Event> {
        let mut result = vec![];
        let mut at = self.head;
        while let Some(index) = at {
            let node = self.nodes[index].as_ref().unwrap();
            result.push(node.event);
            at = node.next;
        }
        result
    }

    pub fn tick(&mut self, host: &mut impl Host) -> Result<(), String> {
        if self.ticking {
            return Err("recursive script-event traversal is unsupported".into());
        }
        self.ticking = true;
        let result = self.tick_inner(host);
        self.ticking = false;
        result
    }

    fn tick_inner(&mut self, host: &mut impl Host) -> Result<(), String> {
        let mut at = self.head;
        while let Some(index) = at {
            let node = self.nodes[index].as_mut().unwrap();
            // Original captures next before the Lua callback. Appending at the
            // old tail is therefore deferred, but appends before reaching that
            // tail can be encountered during this very pass.
            at = node.next;
            node.event.remaining = node.event.remaining.wrapping_sub(1);
            if node.event.remaining != 0 {
                continue;
            }
            let event = node.event;
            let Some(delay) = host.invoke(event, self)? else {
                break;
            };
            let delay = crate::economy::ftol_low_word(delay) as u32;
            self.nodes[index].as_mut().unwrap().event.remaining = delay;
            if delay != 0 {
                continue;
            }
            let node = self.nodes[index].take().unwrap();
            if let Some(previous) = node.previous {
                self.nodes[previous].as_mut().unwrap().next = node.next;
            } else {
                self.head = node.next;
            }
            if let Some(next) = node.next {
                self.nodes[next].as_mut().unwrap().previous = node.previous;
            } else {
                self.tail = node.previous;
            }
            self.free.push(index);
            host.release(node.event.reference)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Callback {
        reference: u32,
        result: f64,
        mode: u32,
        append: Event,
    }
    struct FixtureHost {
        callbacks: Vec<Callback>,
        trace: Vec<(u32, u32)>,
    }
    impl Host for FixtureHost {
        fn invoke(&mut self, event: Event, events: &mut Events) -> Result<Option<f64>, String> {
            self.trace.push((1, event.reference));
            let callback = self
                .callbacks
                .iter()
                .find(|c| c.reference == event.reference)
                .unwrap();
            if callback.mode == 1 {
                return Ok(None);
            }
            if callback.mode == 2 {
                events.append(callback.append);
            }
            Ok(Some(callback.result))
        }
        fn release(&mut self, reference: u32) -> Result<(), String> {
            self.trace.push((2, reference));
            Ok(())
        }
    }
    struct Reader<'a>(&'a [u8]);
    impl Reader<'_> {
        fn word(&mut self) -> u32 {
            let (head, tail) = self.0.split_at(4);
            self.0 = tail;
            u32::from_le_bytes(head.try_into().unwrap())
        }
        fn number(&mut self) -> f64 {
            let (head, tail) = self.0.split_at(8);
            self.0 = tail;
            f64::from_le_bytes(head.try_into().unwrap())
        }
        fn event(&mut self) -> Event {
            Event {
                reference: self.word(),
                remaining: self.word(),
                argument: self.word() as i32,
            }
        }
    }
    #[test]
    #[ignore = "requires supplied-original native script event fixture"]
    fn matches_original_countdown_traversal() {
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_SCRIPT_EVENTS").unwrap()).unwrap();
        assert_eq!(&data[..8], b"MTSEVTQ1");
        let mut reader = Reader(&data[8..]);
        let count = reader.word();
        assert!(count >= 6);
        for case in 0..count {
            let initial = reader.word();
            let ticks = reader.word();
            let callbacks = reader.word();
            let trace = reader.word();
            let final_count = reader.word();
            let mut events = Events::default();
            for _ in 0..initial {
                events.append(reader.event());
            }
            let callbacks = (0..callbacks)
                .map(|_| Callback {
                    reference: reader.word(),
                    result: reader.number(),
                    mode: reader.word(),
                    append: reader.event(),
                })
                .collect();
            let expected_trace: Vec<_> =
                (0..trace).map(|_| (reader.word(), reader.word())).collect();
            let expected: Vec<_> = (0..final_count).map(|_| reader.event()).collect();
            let mut host = FixtureHost {
                callbacks,
                trace: vec![],
            };
            for _ in 0..ticks {
                events.tick(&mut host).unwrap();
            }
            assert_eq!(host.trace, expected_trace, "case {case} calls");
            assert_eq!(events.pending(), expected, "case {case} records");
        }
        assert!(reader.0.is_empty());
    }
}
