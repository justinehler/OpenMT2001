//! Synthetic transport tests only: no bank math, clock, original data or renderer.
use super::*;

fn pose(mark: u32) -> Pose {
    Pose {
        eye: [mark; 3],
        target: [mark.wrapping_add(1); 3],
        up: [0, 0x3f800000, 0],
    }
}
fn observation(boundary: Boundary, mark: u32) -> Observation {
    Observation {
        raw_capture: None,
        strategic_writeback: None,
        finalization_fault: None,
        current: pose(mark),
        sampled: matches!(boundary, Boundary::Sample(_)).then(|| pose(mark.wrapping_add(2))),
        task_effects: vec![],
    }
}
fn install() -> Operation {
    // Opaque pair includes different unknown modes/secondary bytes. Transport
    // must not require the finite numeric core's mode set or rewrite either row.
    let mut records = [[0xa5; 196], [0x5a; 196]];
    records[0][..4].copy_from_slice(&u32::MAX.to_le_bytes());
    Operation::Install {
        property: None,
        capture: Capture::from_records(records),
        admission: Admission::default(),
    }
}
fn inject(
    inbox: &mut Inbox,
    sender: &mpsc::Sender<Request>,
    generation: u64,
    sequence: u64,
    operation: Operation,
    apply: impl FnOnce(Operation) -> Result<Observation, String>,
) -> (Result<Service, String>, Result<Ack, String>) {
    let (reply, response) = mpsc::channel();
    sender
        .send(Request {
            generation,
            sequence,
            operation,
            reply,
        })
        .unwrap();
    (
        inbox.try_service(apply),
        match response.recv().unwrap() {
            Reply::Complete(result) => result.map_err(|error| error.to_string()),
            Reply::Session(_) => panic!("unexpected callback in camera-only fixture"),
        },
    )
}

#[test]
fn direct_round_trip_preserves_boundaries_and_actual_ratio_bits() {
    let (mut worker, mut inbox) = channel(71);
    let operations = vec![
        install(),
        Operation::CentralTail {
            admission: Admission::default(),
        },
        Operation::Apply {
            operation: NativeOperation::SetKind(13),
            admission: Admission::default(),
        },
        Operation::CurrentPose,
        Operation::CentralTail {
            admission: Admission::default(),
        },
        // Outside [0,1] is still transported verbatim: owner decides support.
        Operation::Sample {
            ratio_bits: 0x3fc00000,
        },
    ];
    let expected = operations.clone();
    let client = std::thread::spawn(move || {
        for (index, operation) in operations.into_iter().enumerate() {
            let boundary = operation.boundary();
            assert_eq!(
                worker.request(operation).unwrap(),
                observation(boundary, index as u32)
            );
        }
    });
    let mut trace = vec![];
    for index in 0..expected.len() {
        loop {
            match inbox
                .try_service(|operation| {
                    let boundary = operation.boundary();
                    trace.push(operation);
                    Ok(observation(boundary, index as u32))
                })
                .unwrap()
            {
                Service::Completed => break,
                Service::Idle => std::thread::yield_now(),
            }
        }
    }
    client.join().unwrap();
    assert_eq!(trace, expected);
}

#[test]
fn empty_inbox_never_fabricates_a_callback_or_sample() {
    let (_worker, mut inbox) = channel(1);
    assert_eq!(
        inbox
            .try_service(|_| panic!("idle camera advanced"))
            .unwrap(),
        Service::Idle
    );
}

#[test]
fn wrong_generation_or_sequence_is_rejected_before_owner_mutation() {
    for (generation, sequence) in [(2, 0), (1, 1)] {
        let (worker, mut inbox) = channel(1);
        let (result, ack) = inject(
            &mut inbox,
            &worker.sender,
            generation,
            sequence,
            install(),
            |_| panic!("protocol error reached bank"),
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("generation/sequence")
        );
        assert!(ack.unwrap_err().to_string().contains("generation/sequence"));
        assert!(
            inbox
                .try_service(|_| panic!("latched failure reached bank"))
                .is_err()
        );
    }
}

#[test]
fn install_is_required_and_cannot_repeat() {
    let (mut worker, _inbox) = channel(1);
    assert!(
        worker
            .request(Operation::CurrentPose)
            .unwrap_err()
            .to_string()
            .contains("initial Install")
    );
    // Local protocol failure also latches; a later Install cannot reopen it.
    assert!(
        worker
            .request(install())
            .unwrap_err()
            .to_string()
            .contains("initial Install")
    );
    let (worker, mut inbox) = channel(3);
    let (_, ack) = inject(&mut inbox, &worker.sender, 3, 0, install(), |operation| {
        Ok(observation(operation.boundary(), 1))
    });
    assert!(ack.is_ok());
    let (_, ack) = inject(&mut inbox, &worker.sender, 3, 1, install(), |_| {
        panic!("duplicate Install mutated bank")
    });
    assert!(ack.unwrap_err().to_string().contains("initial Install"));
}

#[test]
fn operation_failure_returns_direct_error_and_latches_both_endpoints() {
    let (mut worker, mut inbox) = channel(1);
    let client = std::thread::spawn(move || {
        let error = worker.request(install()).unwrap_err();
        assert_eq!(
            error,
            RequestError::Failed("unsupported original operation".into())
        );
        assert_eq!(worker.request(Operation::CurrentPose).unwrap_err(), error);
    });
    loop {
        match inbox.try_service(|_| Err("unsupported original operation".into())) {
            Ok(Service::Idle) => std::thread::yield_now(),
            Err(error) => {
                assert_eq!(error, "unsupported original operation");
                break;
            }
            Ok(Service::Completed) => panic!("failure fabricated success"),
        }
    }
    assert!(
        inbox
            .try_service(|_| panic!("failed owner was retried"))
            .is_err()
    );
    client.join().unwrap();
}

#[test]
fn dropped_inbox_or_reply_never_falls_back() {
    let (mut worker, inbox) = channel(1);
    drop(inbox);
    assert!(
        worker
            .request(install())
            .unwrap_err()
            .to_string()
            .contains("disconnected")
    );
    let (mut worker, inbox) = channel(1);
    let server = std::thread::spawn(move || {
        drop(inbox.receiver.recv().unwrap());
    });
    assert!(
        worker
            .request(install())
            .unwrap_err()
            .to_string()
            .contains("dropped acknowledgement")
    );
    server.join().unwrap();
    let (worker, mut inbox) = channel(1);
    drop(worker);
    assert!(
        inbox
            .try_service(|_| panic!("disconnected worker advanced camera"))
            .unwrap_err()
            .to_string()
            .contains("worker disconnected")
    );
}

#[test]
fn forged_ack_identity_or_boundary_is_rejected() {
    for field in 0..4 {
        let (mut worker, inbox) = channel(7);
        let server = std::thread::spawn(move || {
            let request = inbox.receiver.recv().unwrap();
            let mut ack = Ack {
                generation: request.generation,
                sequence: request.sequence,
                boundary: Boundary::Install,
                observation: observation(Boundary::Install, 1),
                subcalls: 0,
            };
            match field {
                0 => ack.generation += 1,
                1 => ack.sequence += 1,
                2 => ack.boundary = Boundary::CurrentPose,
                _ => ack.subcalls += 1,
            }
            request.reply.send(Reply::Complete(Ok(ack))).unwrap();
        });
        assert!(
            worker
                .request(install())
                .unwrap_err()
                .to_string()
                .contains("mismatch")
        );
        assert!(worker.request(Operation::CurrentPose).is_err());
        server.join().unwrap();
    }
}

#[test]
fn malformed_sample_and_read_task_mutations_are_protocol_errors() {
    let (worker, mut inbox) = channel(3);
    let (_, ack) = inject(&mut inbox, &worker.sender, 3, 0, install(), |operation| {
        Ok(observation(operation.boundary(), 1))
    });
    assert!(ack.is_ok());
    let (_, ack) = inject(
        &mut inbox,
        &worker.sender,
        3,
        1,
        Operation::Sample {
            ratio_bits: 0x3f000000,
        },
        |_| Ok(observation(Boundary::CurrentPose, 2)),
    );
    assert!(ack.unwrap_err().to_string().contains("sampled observation"));

    let (worker, mut inbox) = channel(3);
    let (_, ack) = inject(&mut inbox, &worker.sender, 3, 0, install(), |operation| {
        Ok(observation(operation.boundary(), 1))
    });
    assert!(ack.is_ok());
    let (_, ack) = inject(
        &mut inbox,
        &worker.sender,
        3,
        1,
        Operation::CurrentPose,
        |operation| {
            let mut output = observation(operation.boundary(), 2);
            output.task_effects.push(TaskEffect::ClearElapsed);
            Ok(output)
        },
    );
    assert!(ack.unwrap_err().to_string().contains("Only CentralTail"));
}

#[test]
fn acknowledged_central_returns_ordered_task_effects() {
    let (worker, mut inbox) = channel(3);
    let (_, ack) = inject(&mut inbox, &worker.sender, 3, 0, install(), |operation| {
        Ok(observation(operation.boundary(), 1))
    });
    assert!(ack.is_ok());
    let effects = vec![
        TaskEffect::SetEnabled {
            mask: 2,
            enabled: false,
        },
        TaskEffect::ClearElapsed,
        TaskEffect::SetEnabled {
            mask: 1,
            enabled: true,
        },
    ];
    let (_, ack) = inject(
        &mut inbox,
        &worker.sender,
        3,
        1,
        Operation::CentralTail {
            admission: Admission::default(),
        },
        |operation| {
            let mut output = observation(operation.boundary(), 2);
            output.task_effects = effects.clone();
            Ok(output)
        },
    );
    assert_eq!(ack.unwrap().observation.task_effects, effects);
}

#[test]
fn exhausted_sequence_stops_before_mutating_owner() {
    let (mut worker, _inbox) = channel(1);
    worker.next = u64::MAX;
    assert!(
        worker
            .request(install())
            .unwrap_err()
            .to_string()
            .contains("sequence exhausted")
    );
    let (worker, mut inbox) = channel(1);
    inbox.next = u64::MAX;
    let (_, ack) = inject(&mut inbox, &worker.sender, 1, u64::MAX, install(), |_| {
        panic!("exhausted sequence reached bank")
    });
    assert!(ack.unwrap_err().to_string().contains("sequence exhausted"));
}

#[test]
fn dropped_direct_receiver_stops_after_the_completed_owner_operation() {
    let (worker, mut inbox) = channel(1);
    let (reply, response) = mpsc::channel();
    drop(response);
    worker
        .sender
        .send(Request {
            generation: 1,
            sequence: 0,
            operation: install(),
            reply,
        })
        .unwrap();
    let mut calls = 0;
    assert!(
        inbox
            .try_service(|operation| {
                calls += 1;
                Ok(observation(operation.boundary(), 1))
            })
            .unwrap_err()
            .to_string()
            .contains("dropped direct reply")
    );
    assert_eq!(calls, 1);
    assert!(
        inbox
            .try_service(|_| panic!("partially completed operation replayed"))
            .is_err()
    );
}

#[test]
fn rendezvous_idle_and_disconnect_never_dispatch_owner() {
    let (worker, mut inbox) = channel(99);
    assert_eq!(
        inbox
            .wait_service(std::time::Duration::ZERO, |_| panic!(
                "fabricated camera work"
            ))
            .unwrap(),
        Service::Idle
    );
    drop(worker);
    assert!(
        inbox
            .wait_service(std::time::Duration::ZERO, |_| panic!(
                "disconnected camera work"
            ))
            .is_err()
    );
}
#[test]
fn rendezvous_services_serial_worker_continuations_in_exact_order() {
    let (mut worker, mut inbox) = channel(100);
    let client = std::thread::spawn(move || {
        worker.request(install()).unwrap();
        for _ in 0..32 {
            worker.request(Operation::CurrentPose).unwrap();
        }
    });
    let mut count = 0;
    for _ in 0..33 {
        assert_eq!(
            inbox
                .wait_service(std::time::Duration::from_secs(2), |op| {
                    let boundary = op.boundary();
                    assert_eq!(matches!(op, Operation::Install { .. }), count == 0);
                    count += 1;
                    Ok(observation(boundary, count))
                })
                .unwrap(),
            Service::Completed
        );
    }
    client.join().unwrap();
    assert_eq!(count, 33);
}

#[test]
fn explicit_retirement_wakes_queued_request_and_rejects_future_operations() {
    let (mut worker, inbox) = channel(201);
    let sender = worker.sender.clone();
    let (done, finished) = mpsc::channel();
    let client = std::thread::spawn(move || {
        let result = worker.request(install());
        done.send((worker, result)).unwrap();
    });
    // Synchronize on the actual request, then put it back in the queue that
    // retirement owns/drops. No scheduling delay is used as proof of waiting.
    let pending = inbox
        .receiver
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    sender.send(pending).unwrap();
    inbox.retire();
    let (mut worker, result) = finished
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    assert_eq!(result, Err(RequestError::Retired));
    assert!(worker.retired());
    assert!(!worker.installed());
    assert!(worker.failure.is_none());
    assert_eq!(
        worker.next, 0,
        "cancelled Install has no ack or sequence commit"
    );
    for operation in [
        install(),
        Operation::CurrentPose,
        Operation::CentralTail {
            admission: Admission::default(),
        },
        Operation::Sample {
            ratio_bits: 0x3f000000,
        },
    ] {
        assert_eq!(worker.request(operation), Err(RequestError::Retired));
    }
    client.join().unwrap();
}

#[test]
fn retirement_is_per_channel_even_when_generation_numbers_are_reused() {
    let (mut worker, inbox) = channel(202);
    let (other, mut live) = channel(202);
    inbox.retire();
    assert_eq!(worker.request(install()), Err(RequestError::Retired));
    assert!(!other.retired());
    assert_eq!(
        live.try_service(|_| panic!("retirement invented work"))
            .unwrap(),
        Service::Idle
    );
}

#[test]
fn retirement_after_ack_keeps_committed_observation_and_task_effects() {
    let (mut worker, mut inbox) = channel(203);
    let (done, finished) = mpsc::channel();
    let client = std::thread::spawn(move || {
        worker.request(install()).unwrap();
        let result = worker.request(Operation::CentralTail {
            admission: Admission::default(),
        });
        done.send((worker, result)).unwrap();
    });
    inbox
        .wait_service(std::time::Duration::from_secs(2), |op| {
            Ok(observation(op.boundary(), 1))
        })
        .unwrap();
    let effects = vec![
        TaskEffect::ClearElapsed,
        TaskEffect::SetEnabled {
            mask: 2,
            enabled: false,
        },
    ];
    inbox
        .wait_service(std::time::Duration::from_secs(2), |op| {
            let mut value = observation(op.boundary(), 2);
            value.task_effects = effects.clone();
            Ok(value)
        })
        .unwrap();
    inbox.retire();
    let (mut worker, result) = finished
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    assert_eq!(result.unwrap().task_effects, effects);
    assert_eq!(worker.next, 2);
    assert_eq!(
        worker.request(Operation::CurrentPose),
        Err(RequestError::Retired)
    );
    client.join().unwrap();
}

#[test]
fn actual_owner_error_is_not_hidden_by_subsequent_retirement() {
    let (mut worker, mut inbox) = channel(204);
    let (done, finished) = mpsc::channel();
    let client = std::thread::spawn(move || {
        let error = worker.request(install()).unwrap_err();
        done.send((worker, error)).unwrap();
    });
    let expected = "real owner failure while city is closing";
    assert_eq!(
        inbox
            .wait_service(std::time::Duration::from_secs(2), |_| Err(expected.into()))
            .unwrap_err(),
        expected
    );
    inbox.retire();
    let (mut worker, error) = finished
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    assert_eq!(error, RequestError::Failed(expected.into()));
    assert_eq!(worker.request(install()), Err(error));
    client.join().unwrap();
}

#[test]
fn forged_ack_remains_failure_when_explicit_retirement_races_receipt() {
    let (mut worker, inbox) = channel(205);
    let server = std::thread::spawn(move || {
        let request = inbox
            .receiver
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        request
            .reply
            .send(Reply::Complete(Ok(Ack {
                generation: 999,
                sequence: request.sequence,
                boundary: Boundary::Install,
                observation: observation(Boundary::Install, 0),
                subcalls: 0,
            })))
            .unwrap();
        inbox.retire();
    });
    assert!(
        matches!(worker.request(install()), Err(RequestError::Failed(error))
        if error.contains("mismatch"))
    );
    server.join().unwrap();
    assert!(matches!(
        worker.request(install()),
        Err(RequestError::Failed(_))
    ));
}

#[test]
fn unexpected_drop_is_typed_failure_and_never_sets_retirement() {
    let (mut worker, inbox) = channel(206);
    drop(inbox);
    let error = worker.request(install()).unwrap_err();
    assert!(matches!(&error, RequestError::Failed(message) if message.contains("disconnected")));
    assert!(!worker.retired());
    assert_eq!(worker.request(install()), Err(error));
}

#[test]
fn completed_fault_direct_ack_survives_retirement_and_latches_real_failure() {
    let (mut worker, mut inbox) = channel(802);
    let client = std::thread::spawn(move || {
        worker.request(install()).unwrap();
        let completed = worker
            .request(Operation::CentralTail {
                admission: Admission::default(),
            })
            .unwrap();
        assert_eq!(completed.task_effects, vec![TaskEffect::ClearElapsed]);
        assert_eq!(
            completed.finalization_fault.as_deref(),
            Some("late extractor failed")
        );
        assert_eq!(
            worker.request(Operation::CurrentPose),
            Err(RequestError::Failed("late extractor failed".into()))
        );
    });
    inbox
        .wait_service(std::time::Duration::from_secs(2), |_| {
            Ok(observation(Boundary::Install, 1))
        })
        .unwrap();
    let result = inbox.wait_service(std::time::Duration::from_secs(2), |_| {
        let mut o = observation(Boundary::CentralTail, 2);
        o.task_effects.push(TaskEffect::ClearElapsed);
        o.finalization_fault = Some("late extractor failed".into());
        Ok(o)
    });
    assert_eq!(result, Err("late extractor failed".into()));
    inbox.retire();
    client.join().unwrap();
}
#[test]
fn completion_fault_is_rejected_outside_protected_completion() {
    for boundary in [
        Boundary::Admission,
        Boundary::Apply,
        Boundary::CurrentPose,
        Boundary::Sample(0),
    ] {
        let mut o = observation(boundary, 1);
        o.finalization_fault = Some("wrong boundary".into());
        assert!(validate_observation(boundary, &o).is_err());
    }
}

// The operation uses existing envelope, type and retirement checks.
#[test]
fn raw_capture_reply_is_typed_and_read_only() {
    let mut raw = observation(Boundary::CurrentPose, 0);
    assert!(validate_observation(Boundary::RawCapture, &raw).is_err());
    raw.raw_capture = Some(SaveCapture {
        cameras: Capture::from_records([[0xa5; 196], [0x5a; 196]]),
        interface_mode: Ok(3),
    });
    assert!(validate_observation(Boundary::RawCapture, &raw).is_ok());
    assert!(validate_observation(Boundary::CurrentPose, &raw).is_err());
    raw.task_effects.push(TaskEffect::ClearElapsed);
    assert!(validate_observation(Boundary::RawCapture, &raw).is_err());
    raw.task_effects.clear();
    raw.sampled = Some(pose(7));
    assert!(validate_observation(Boundary::RawCapture, &raw).is_err());
    raw.sampled = None;
    raw.finalization_fault = Some("forged finalizer".into());
    assert!(validate_observation(Boundary::RawCapture, &raw).is_err());
}
#[test]
fn raw_capture_rejects_bad_generation_and_sequence_before_capture() {
    for (generation, sequence) in [(2, 0), (1, 1)] {
        let (worker, mut inbox) = channel(1);
        let (result, ack) = inject(
            &mut inbox,
            &worker.sender,
            generation,
            sequence,
            Operation::RawCapture,
            |_| panic!("invalid envelope reached raw owner"),
        );
        assert!(result.unwrap_err().contains("generation/sequence"));
        assert!(ack.unwrap_err().contains("generation/sequence"));
    }
}
#[test]
fn raw_capture_retirement_is_not_a_successful_save_or_fatal_camera_error() {
    let (mut worker, inbox) = channel(211);
    worker.installed = true;
    let sender = worker.sender.clone();
    let (done, finished) = mpsc::channel();
    let client = std::thread::spawn(move || {
        let result = worker.request(Operation::RawCapture);
        done.send((worker, result)).unwrap();
    });
    let pending = inbox
        .receiver
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    sender.send(pending).unwrap();
    inbox.retire();
    let (mut worker, result) = finished
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    assert_eq!(result, Err(RequestError::Retired));
    assert_eq!(worker.next, 0);
    assert!(worker.failure.is_none());
    assert_eq!(
        worker.request(Operation::RawCapture),
        Err(RequestError::Retired)
    );
    client.join().unwrap();
}
