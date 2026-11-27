//! Real operation/reply channels with synthetic observations.
//! No sleeps, prequeued callback fiction, bank math, clocks or native input.
use super::*;
use std::{thread, time::Duration};
const WAIT: Duration = Duration::from_secs(3);
fn observation() -> Observation {
    Observation {
        raw_capture: None,
        current: Pose {
            eye: [0; 3],
            target: [0; 3],
            up: [0; 3],
        },
        sampled: None,
        strategic_writeback: None,
        finalization_fault: None,
        task_effects: vec![],
    }
}
fn central() -> Operation {
    Operation::CentralTail {
        admission: Admission::default(),
    }
}
fn installed() -> (Worker, Inbox) {
    let (mut worker, mut inbox) = channel(601);
    let ui = thread::spawn(move || {
        assert_eq!(
            inbox
                .wait_service(WAIT, |operation| {
                    assert!(matches!(operation, Operation::Install { .. }));
                    Ok(observation())
                })
                .unwrap(),
            Service::Completed
        );
        inbox
    });
    worker
        .request(Operation::Install {
            property: None,
            capture: Capture::from_records([[0; 196]; 2]),
            admission: Admission::default(),
        })
        .unwrap();
    (worker, ui.join().unwrap())
}
#[test]
fn actual_request_services_in_order_and_next_read_sees_committed_effect() {
    let (mut worker, mut inbox) = installed();
    let ui = thread::spawn(move || {
        inbox
            .wait_service_with_handler(WAIT, |_, port| {
                assert_eq!(port.pause_byte(PauseByte::SignedReason3_8b22d3).unwrap(), 0);
                port.pause4bac90(PauseReason::Action3).unwrap();
                assert_eq!(port.pause_byte(PauseByte::SignedReason3_8b22d3).unwrap(), 1);
                Ok(observation())
            })
            .unwrap()
    });
    let mut actual = 0u8;
    let mut calls = vec![];
    worker
        .request_with_handler(central(), |call| {
            calls.push(call.clone());
            Ok(match call {
                SessionCall::PauseByte(_) => SessionReply::Byte(actual),
                SessionCall::Pause4bac90(_) => {
                    actual += 1;
                    SessionReply::Completed
                }
                _ => panic!("unexpected call"),
            })
        })
        .unwrap();
    assert_eq!(actual, 1);
    assert_eq!(calls.len(), 3);
    assert_eq!(ui.join().unwrap(), Service::Completed);
    assert_eq!(worker.next, 2);
}
#[test]
fn late_ui_error_keeps_prior_host_commit_and_latches_both_ends() {
    let (mut worker, mut inbox) = installed();
    let ui = thread::spawn(move || {
        let failed = inbox
            .wait_service_with_handler(WAIT, |_, port| {
                port.pause4bac90(PauseReason::Action3).unwrap();
                Err("late UI fault".into())
            })
            .unwrap_err();
        assert_eq!(failed, "late UI fault");
        assert!(
            inbox
                .try_service(|_| panic!("replayed UI operation"))
                .is_err()
        );
    });
    let mut committed = 0;
    let error = worker
        .request_with_handler(central(), |_| {
            committed += 1;
            Ok(SessionReply::Completed)
        })
        .unwrap_err();
    assert_eq!(committed, 1);
    assert_eq!(error, RequestError::Failed("late UI fault".into()));
    assert_eq!(worker.request(central()), Err(error));
    ui.join().unwrap();
}
#[test]
fn completed_task_effects_and_prior_host_commit_survive_late_finalizer_fault() {
    let (mut worker, mut inbox) = installed();
    let ui = thread::spawn(move || {
        assert_eq!(
            inbox
                .wait_service_with_handler(WAIT, |_, port| {
                    port.pause4bac90(PauseReason::Action3).unwrap();
                    let mut ack = observation();
                    ack.task_effects.push(TaskEffect::ClearElapsed);
                    ack.finalization_fault = Some("late finalizer".into());
                    Ok(ack)
                })
                .unwrap_err(),
            "late finalizer"
        );
    });
    let mut committed = false;
    let ack = worker
        .request_with_handler(central(), |_| {
            committed = true;
            Ok(SessionReply::Completed)
        })
        .unwrap();
    assert!(committed);
    assert_eq!(ack.task_effects, vec![TaskEffect::ClearElapsed]);
    assert_eq!(ack.finalization_fault.as_deref(), Some("late finalizer"));
    assert_eq!(
        worker.request(central()),
        Err(RequestError::Failed("late finalizer".into()))
    );
    ui.join().unwrap();
}
#[test]
fn camera_only_request_rejects_missing_handler_and_cannot_ignore_service_failure() {
    let (mut worker, mut inbox) = installed();
    let ui = thread::spawn(move || {
        let error = inbox
            .wait_service_with_handler(WAIT, |_, port| {
                assert!(port.pause4bac90(PauseReason::Action3).is_err());
                // Ignoring the failure cannot manufacture an operation ack.
                assert!(port.busy48dbf0().is_err());
                Ok(observation())
            })
            .unwrap_err();
        assert!(error.contains("handler absent"));
    });
    let error = worker.request(central()).unwrap_err();
    assert!(error.to_string().contains("handler absent"));
    assert_eq!(worker.request(central()), Err(error));
    ui.join().unwrap();
}
#[test]
fn handler_failure_is_sticky_and_never_replayed_even_if_ui_swallows_it() {
    let (mut worker, mut inbox) = installed();
    let ui = thread::spawn(move || {
        assert_eq!(
            inbox
                .wait_service_with_handler(WAIT, |_, port| {
                    assert!(port.pause4bac90(PauseReason::Action3).is_err());
                    assert!(port.pause4bac90(PauseReason::Action3).is_err());
                    Ok(observation())
                })
                .unwrap_err(),
            "handler partial fault"
        );
    });
    let mut committed = 0;
    let error = worker
        .request_with_handler(central(), |_| {
            committed += 1;
            Err("handler partial fault".into())
        })
        .unwrap_err();
    assert_eq!(committed, 1);
    assert_eq!(error, RequestError::Failed("handler partial fault".into()));
    ui.join().unwrap();
}
#[test]
fn callbacks_outside_central_are_rejected_before_host_handler() {
    let (mut worker, mut inbox) = installed();
    let ui = thread::spawn(move || {
        assert!(
            inbox
                .wait_service_with_handler(WAIT, |_, port| {
                    assert!(port.busy48dbf0().is_err());
                    Ok(observation())
                })
                .unwrap_err()
                .contains("outside admitted central")
        );
    });
    assert!(
        worker
            .request_with_handler(Operation::CurrentPose, |_| panic!(
                "inadmissible callback reached Host"
            ))
            .is_err()
    );
    ui.join().unwrap();
}
#[test]
fn retired_pending_operation_runs_no_callback() {
    let (mut worker, inbox) = installed();
    let (enqueued, queued) = mpsc::channel();
    worker.test_notify_next_enqueue(enqueued);
    let client = thread::spawn(move || {
        worker.request_with_handler(central(), |_| panic!("callback after retirement"))
    });
    queued.recv_timeout(WAIT).unwrap(); //Actual successful request enqueue.
    inbox.retire();
    assert_eq!(client.join().unwrap(), Err(RequestError::Retired));
}
#[test]
fn unexpected_pending_disconnect_is_failure_not_retirement() {
    let (mut worker, inbox) = installed();
    let (enqueued, queued) = mpsc::channel();
    worker.test_notify_next_enqueue(enqueued);
    let client = thread::spawn(move || {
        let error = worker
            .request_with_handler(central(), |_| panic!("callback after drop"))
            .unwrap_err();
        assert!(matches!(error, RequestError::Failed(_)));
        assert!(!worker.retired());
        assert_eq!(worker.request(central()), Err(error));
    });
    queued.recv_timeout(WAIT).unwrap();
    drop(inbox);
    client.join().unwrap();
}
#[test]
fn stale_or_cross_operation_service_rejected_before_callback_and_latched() {
    for field in 0..4 {
        let (mut worker, inbox) = installed();
        let ui = thread::spawn(move || {
            let request = inbox.receiver.recv_timeout(WAIT).unwrap();
            let mut id = CallId {
                generation: request.generation,
                sequence: request.sequence,
                boundary: Boundary::CentralTail,
                subcall: 0,
            };
            match field {
                0 => id.generation += 1,
                1 => id.sequence += 1,
                2 => id.boundary = Boundary::CurrentPose,
                _ => id.subcall += 1,
            }
            let (reply, response) = mpsc::channel();
            request
                .reply
                .send(Reply::Session(SessionRequest {
                    id,
                    call: SessionCall::Busy48dbf0,
                    reply,
                }))
                .unwrap();
            assert!(response.recv_timeout(WAIT).unwrap().result.is_err());
            request
                .reply
                .send(Reply::Complete(Ok(Ack {
                    generation: request.generation,
                    sequence: request.sequence,
                    boundary: Boundary::CentralTail,
                    subcalls: 0,
                    observation: observation(),
                })))
                .unwrap();
        });
        let error = worker
            .request_with_handler(central(), |_| panic!("forged subcall reached handler"))
            .unwrap_err();
        assert!(error.to_string().contains("mismatch"));
        assert_eq!(worker.request(central()), Err(error));
        ui.join().unwrap();
    }
}
#[test]
fn duplicate_subcall_never_reenters_or_replays_host_effect() {
    let (mut worker, inbox) = installed();
    let ui = thread::spawn(move || {
        let request = inbox.receiver.recv_timeout(WAIT).unwrap();
        let id = CallId {
            generation: request.generation,
            sequence: request.sequence,
            boundary: Boundary::CentralTail,
            subcall: 0,
        };
        for duplicate in [false, true] {
            let (reply, response) = mpsc::channel();
            request
                .reply
                .send(Reply::Session(SessionRequest {
                    id,
                    call: SessionCall::Pause4bac90(PauseReason::Action3),
                    reply,
                }))
                .unwrap();
            assert_eq!(
                response.recv_timeout(WAIT).unwrap().result.is_err(),
                duplicate
            );
        }
        request
            .reply
            .send(Reply::Complete(Ok(Ack {
                generation: request.generation,
                sequence: request.sequence,
                boundary: Boundary::CentralTail,
                subcalls: 1,
                observation: observation(),
            })))
            .unwrap();
    });
    let mut committed = 0;
    assert!(
        worker
            .request_with_handler(central(), |_| {
                committed += 1;
                Ok(SessionReply::Completed)
            })
            .is_err()
    );
    assert_eq!(committed, 1);
    ui.join().unwrap();
}
#[test]
fn no_callback_can_be_delivered_after_operation_ack() {
    let (mut worker, inbox) = installed();
    let (finished, returned) = mpsc::channel();
    let ui = thread::spawn(move || {
        let request = inbox.receiver.recv_timeout(WAIT).unwrap();
        request
            .reply
            .send(Reply::Complete(Ok(Ack {
                generation: request.generation,
                sequence: request.sequence,
                boundary: Boundary::CentralTail,
                subcalls: 0,
                observation: observation(),
            })))
            .unwrap();
        returned.recv_timeout(WAIT).unwrap(); //Actual request return, no timing assumption.
        let (reply, _response) = mpsc::channel();
        assert!(
            request
                .reply
                .send(Reply::Session(SessionRequest {
                    id: CallId {
                        generation: request.generation,
                        sequence: request.sequence,
                        boundary: Boundary::CentralTail,
                        subcall: 0
                    },
                    call: SessionCall::Busy48dbf0,
                    reply
                }))
                .is_err()
        );
    });
    worker
        .request_with_handler(central(), |_| panic!("callback after ack"))
        .unwrap();
    finished.send(()).unwrap();
    ui.join().unwrap();
}
#[test]
fn ui_validates_callback_reply_identity_and_type_before_next_read() {
    for field in 0..5 {
        let (sender, stream) = mpsc::channel();
        let retired = AtomicBool::new(false);
        let host = thread::spawn(move || {
            let Reply::Session(request) = stream.recv_timeout(WAIT).unwrap() else {
                panic!("expected service")
            };
            let mut id = request.id;
            match field {
                0 => id.generation += 1,
                1 => id.sequence += 1,
                2 => id.boundary = Boundary::Sample(0),
                3 => id.subcall += 1,
                _ => {}
            }
            request
                .reply
                .send(SessionResponse {
                    id,
                    result: Ok(if field == 4 {
                        SessionReply::Completed
                    } else {
                        SessionReply::Byte(7)
                    }),
                })
                .unwrap();
        });
        let mut port = SessionServices {
            id: CallId {
                generation: 1,
                sequence: 2,
                boundary: Boundary::CentralTail,
                subcall: 0,
            },
            next: 0,
            sender: &sender,
            retired: &retired,
            failure: None,
        };
        let error = port.busy48dbf0().unwrap_err();
        assert_eq!(port.busy48dbf0(), Err(error));
        assert_eq!(port.next, 0);
        host.join().unwrap();
    }
}
#[test]
fn retired_generation_rejects_already_queued_callback_with_typed_retirement() {
    let (mut worker, inbox) = installed();
    let ui = thread::spawn(move || {
        let request = inbox.receiver.recv_timeout(WAIT).unwrap();
        let id = CallId {
            generation: request.generation,
            sequence: request.sequence,
            boundary: Boundary::CentralTail,
            subcall: 0,
        };
        inbox.retire(); //Actual sole production retirement writer, before callback dispatch.
        let (reply, response) = mpsc::channel();
        request
            .reply
            .send(Reply::Session(SessionRequest {
                id,
                call: SessionCall::Busy48dbf0,
                reply,
            }))
            .unwrap();
        assert_eq!(
            response.recv_timeout(WAIT).unwrap().result,
            Err(RequestError::Retired)
        );
        request
            .reply
            .send(Reply::Complete(Err(RequestError::Retired)))
            .unwrap();
    });
    assert_eq!(
        worker.request_with_handler(central(), |_| panic!("callback after retirement")),
        Err(RequestError::Retired)
    );
    assert_eq!(worker.request(central()), Err(RequestError::Retired));
    ui.join().unwrap();
}
#[test]
fn callback_failure_is_not_hidden_by_later_explicit_retirement() {
    let (mut worker, inbox) = installed();
    let ui = thread::spawn(move || {
        let request = inbox.receiver.recv_timeout(WAIT).unwrap();
        let id = CallId {
            generation: request.generation,
            sequence: request.sequence,
            boundary: Boundary::CentralTail,
            subcall: 0,
        };
        let (reply, response) = mpsc::channel();
        request
            .reply
            .send(Reply::Session(SessionRequest {
                id,
                call: SessionCall::Pause4bac90(PauseReason::Action3),
                reply,
            }))
            .unwrap();
        assert_eq!(
            response.recv_timeout(WAIT).unwrap().result,
            Err(RequestError::Failed("partial callback failure".into()))
        );
        inbox.retire();
        drop(request); //No completion: remembered failure wins disconnect/cancel.
    });
    let mut committed = 0;
    let error = worker
        .request_with_handler(central(), |_| {
            committed += 1;
            Err("partial callback failure".into())
        })
        .unwrap_err();
    assert_eq!(committed, 1);
    assert_eq!(
        error,
        RequestError::Failed("partial callback failure".into())
    );
    assert_eq!(worker.request(central()), Err(error));
    ui.join().unwrap();
}
#[test]
fn dropped_callback_reply_is_a_sticky_protocol_failure() {
    let (sender, stream) = mpsc::channel();
    let retired = AtomicBool::new(false);
    let host = thread::spawn(move || {
        let Reply::Session(request) = stream.recv_timeout(WAIT).unwrap() else {
            panic!("expected service")
        };
        drop(request);
    });
    let mut port = SessionServices {
        id: CallId {
            generation: 1,
            sequence: 1,
            boundary: Boundary::CentralTail,
            subcall: 0,
        },
        next: 0,
        sender: &sender,
        retired: &retired,
        failure: None,
    };
    let error = port.busy48dbf0().unwrap_err();
    assert!(matches!(error, RequestError::Failed(_)));
    assert_eq!(port.busy48dbf0(), Err(error));
    host.join().unwrap();
}
#[test]
fn invented_retirement_without_actual_inbox_retire_is_a_latched_failure() {
    let (mut worker, mut inbox) = installed();
    let ui = thread::spawn(move || {
        assert!(
            inbox
                .wait_service_with_handler(WAIT, |_, port| {
                    assert!(matches!(port.busy48dbf0(), Err(RequestError::Failed(_))));
                    Ok(observation())
                })
                .is_err()
        );
    });
    let error = worker
        .request_with_handler(central(), |_| Err(RequestError::Retired))
        .unwrap_err();
    assert!(matches!(error, RequestError::Failed(_)));
    assert!(!worker.retired());
    assert_eq!(worker.request(central()), Err(error));
    ui.join().unwrap();
}
