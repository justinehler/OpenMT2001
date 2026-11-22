//! Original local450490/451720/450a30 timing, shared by live command payloads.
//! Network transport remains outside the simulation.
#[derive(Clone)]
struct Timed<T> {
    due: u32,
    command: T,
    /// Kind1 scheduled events dispatch directly instead of450a30 readmission.
    direct: bool,
}

#[cfg(test)]
mod restore_tests {
    use super::*;
    #[test]
    fn saved_nodes_resume_in_original_clock_domain_and_stable_order() {
        let mut queue = Queue::default();
        queue.restore_queued(10, crate::scheduler::ClockDomain::Total, 31, 31, false);
        queue.restore_queued(11, crate::scheduler::ClockDomain::Total, 31, 31, false);
        queue.restore_queued(20, crate::scheduler::ClockDomain::Normal, 8, 8, true);
        let mut seen = Vec::new();
        queue.drain(&mut [30, 7], |command| { seen.push(*command); Ok(()) }).unwrap();
        assert!(seen.is_empty());
        queue.drain(&mut [31, 7], |command| { seen.push(*command); Ok(()) }).unwrap();
        assert_eq!(seen, [10, 11]);
        queue.drain(&mut [31, 8], |command| { seen.push(*command); Ok(()) }).unwrap();
        assert_eq!(seen, [10, 11, 20]);
    }
    #[test]
    fn saved_packet_uses_inner_admission_due_after_outer_queue_due() {
        let mut queue = Queue::default();
        queue.restore_queued(7, crate::scheduler::ClockDomain::Normal, 50, 51, false);
        let mut seen = Vec::new();
        queue.drain(&mut [50, 50], |command| { seen.push(*command); Ok(()) }).unwrap();
        assert!(seen.is_empty(), "450a30 must requeue the packet by its inner due");
        queue.drain(&mut [51, 50], |command| { seen.push(*command); Ok(()) }).unwrap();
        assert_eq!(seen, [7]);
    }
}
#[derive(Clone, Copy)]
pub enum Schedule {
    Local { delayed: bool, additional: u32 },
    Normal { due: u32 },
}
pub struct Queue<T> {
    queue: crate::scheduler::Queue<Timed<T>>,
}
/// Clock domain, outer deadline, admission deadline, direct flag and command.
pub type Snapshot<'a, T> = (crate::scheduler::ClockDomain, u32, u32, bool, &'a T);
impl<T> Default for Queue<T> {
    fn default() -> Self {
        Self {
            queue: crate::scheduler::Queue::default(),
        }
    }
}
impl<T: Clone> Queue<T> {
    /// Preserve outer queue and inner admission deadlines independently.
    pub fn snapshot(&self) -> Result<Vec<Snapshot<'_, T>>, String> {
        Ok(self.queue.snapshot()?.into_iter().map(|(domain, due, row)|
            (domain, due, row.due, row.direct, &row.command)).collect())
    }
    /// Reconstruct an already admitted 4810e0 queue node. The saved header's
    /// +4 word selects the total or normal list and +8 is its original queue
    /// due. Kind-zero packets carry a separate admission due at packet+0xc,
    /// consumed by 450a30 after 480ec0 removes the outer node.
    pub fn restore_queued(
        &mut self,
        command: T,
        domain: crate::scheduler::ClockDomain,
        queue_due: u32,
        admission_due: u32,
        direct: bool,
    ) {
        self.queue.insert(
            domain,
            queue_due,
            Timed {
                due: admission_due,
                command,
                direct,
            },
        );
    }
    fn dispatch(
        queue: &mut crate::scheduler::Queue<Timed<T>>,
        message: Timed<T>,
        tick: u32,
        apply: &mut impl FnMut(&T) -> Result<(), String>,
    ) -> Result<(), String> {
        //450a30 uses unsigned admission, unlike the queue's signed drain gate.
        if message.due != 0 && message.due > tick {
            if message.due == u32::MAX {
                return Err(
                    "original -1 command admission requires unresolved allocator behavior".into(),
                );
            }
            queue.insert(crate::scheduler::ClockDomain::Total, message.due, message);
            Ok(())
        } else {
            apply(&message.command)
        }
    }
    pub fn submit(
        &mut self,
        command: T,
        tick: u32,
        delayed: bool,
        mut apply: impl FnMut(&T) -> Result<(), String>,
    ) -> Result<(), String> {
        self.submit_with_delay(command, tick, delayed, 0, &mut apply)
    }
    ///450490 stores the additional delay before451720 optionally adds tick+25.
    pub fn submit_with_delay(
        &mut self,
        command: T,
        tick: u32,
        delayed: bool,
        additional: u32,
        mut apply: impl FnMut(&T) -> Result<(), String>,
    ) -> Result<(), String> {
        let due = if delayed {
            additional.wrapping_add(tick).wrapping_add(25)
        } else {
            additional
        };
        Self::dispatch(
            &mut self.queue,
            Timed {
                due,
                command,
                direct: false,
            },
            tick,
            &mut apply,
        )
    }
    /// Nested delayed producers insert into the same native ordered queue.
    /// Immediate nested consumers must be executed by the runtime itself.
    pub fn drain_enqueuing(
        &mut self,
        clocks: &mut [u32; 2],
        mut apply: impl FnMut(
            &T,
            &mut dyn FnMut(T, bool, u32) -> Result<(), String>,
        ) -> Result<(), String>,
    ) -> Result<(), String> {
        self.drain_scheduling(clocks, |command, enqueue| {
            apply(command, &mut |command, delayed, additional| {
                enqueue(
                    command,
                    Schedule::Local {
                        delayed,
                        additional,
                    },
                )
            })
        })
    }
    /// Kind1 normal-clock events share the original queue with local commands.
    pub fn drain_scheduling(
        &mut self,
        clocks: &mut [u32; 2],
        mut apply: impl FnMut(
            &T,
            &mut dyn FnMut(T, Schedule) -> Result<(), String>,
        ) -> Result<(), String>,
    ) -> Result<(), String> {
        let mut remaining = 4096usize;
        self.queue.drain(clocks, |message, queue, clocks| {
            if remaining == 0 {
                return Err("local command drain exceeded4096 dispatches".into());
            }
            remaining -= 1;
            if !message.direct && message.due != 0 && message.due > clocks[0] {
                return Self::dispatch(queue, message.clone(), clocks[0], &mut |_| {
                    Err("queued command unexpectedly became immediate".into())
                });
            }
            apply(&message.command, &mut |command, schedule| {
                let Schedule::Local {
                    delayed,
                    additional,
                } = schedule
                else {
                    let Schedule::Normal { due } = schedule else {
                        unreachable!()
                    };
                    if due as i32 <= clocks[1] as i32 {
                        return Err(
                            "original480900 immediate normal-event free fallthrough is unsupported"
                                .into(),
                        );
                    }
                    queue.insert(
                        crate::scheduler::ClockDomain::Normal,
                        due,
                        Timed {
                            due,
                            command,
                            direct: true,
                        },
                    );
                    return Ok(());
                };
                let tick = clocks[0];
                let due = if delayed {
                    additional.wrapping_add(tick).wrapping_add(25)
                } else {
                    additional
                };
                Self::dispatch(
                    queue,
                    Timed {
                        due,
                        command,
                        direct: false,
                    },
                    tick,
                    &mut |_| {
                        Err("nested command requires synchronous consumer after delay wrap".into())
                    },
                )
            })
        })
    }
    pub fn drain(
        &mut self,
        clocks: &mut [u32; 2],
        mut apply: impl FnMut(&T) -> Result<(), String>,
    ) -> Result<(), String> {
        // Signed-boundary events can copy/requeue forever in the original.
        let mut remaining = 4096usize;
        self.queue.drain(clocks, |message,queue,clocks| {
            if remaining==0 {return Err("local command drain exceeded4096 dispatches; original queue may be nonterminating".into());}
            remaining-=1;
            if message.direct{apply(&message.command)}else{Self::dispatch(queue,message.clone(),clocks[0],&mut apply)}
        })
    }
}
