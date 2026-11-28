use super::*;
pub(crate) fn settings() -> Settings {
    Settings {
        name: b"Actual session".to_vec(),
        level: 300001,
        parameter_bits: 2000f32.to_bits(),
        difficulty: 1,
        map: b"default".to_vec(),
    }
}
pub(crate) fn names() -> [Vec<u8>; 10] {
    std::array::from_fn(|i| format!("Character {i}").into_bytes())
}
fn host() -> Snapshot {
    Snapshot::host(settings(), b"Host".to_vec(), 4).unwrap()
}
#[test]
fn rows_permissions_revisions_and_readiness() {
    let mut lobby = host();
    assert!(!lobby.start_enabled());
    lobby.action(Action::Kind(1), &names()).unwrap();
    lobby.action(Action::Difficulty(1), &names()).unwrap();
    assert_eq!(lobby.rows[1].difficulty, 1);
    assert!(lobby.start_enabled());
    lobby
        .request(
            5,
            Request::Join {
                name: b"Client".to_vec(),
                token: 4,
            },
        )
        .unwrap();
    assert_eq!(lobby.local_row(5), Some(2));
    assert_eq!(lobby.rows[0].player.token, 4);
    assert!(!lobby.start_enabled());
    let before = lobby.clone();
    assert!(
        lobby
            .action(
                Action::Character {
                    row: 2,
                    direction: 1
                },
                &names()
            )
            .is_err()
    );
    assert!(lobby.action(Action::Kind(2), &names()).is_err());
    assert!(lobby.action(Action::Difficulty(2), &names()).is_err());
    assert!(
        lobby
            .client_edit(
                5,
                Action::Character {
                    row: 0,
                    direction: 1
                }
            )
            .is_err()
    );
    assert!(lobby.client_edit(5, Action::Kind(3)).is_err());
    assert!(lobby.client_edit(5, Action::Start).is_err());
    assert!(
        lobby
            .request(
                5,
                Request::Edit {
                    token: 4,
                    ready: true
                }
            )
            .is_err()
    );
    assert_eq!(lobby, before);
    let edit = lobby.client_edit(5, Action::Ready).unwrap();
    lobby.request(5, edit).unwrap();
    assert_eq!(lobby.revision, before.revision + 1);
    assert!(lobby.start_enabled());
    lobby.remove(5).unwrap();
    assert_eq!(lobby.rows[2].player.kind, RowKind::Open);
    assert_eq!(lobby.rows[1].difficulty, 1);
    lobby.action(Action::Kind(1), &names()).unwrap();
    assert_eq!(lobby.rows[1].player.kind, RowKind::Closed);
    assert_eq!(lobby.capacity(), (1, 5));
    assert!(!lobby.start_enabled());
    lobby.action(Action::Kind(1), &names()).unwrap();
    assert_eq!(lobby.rows[1].difficulty, 1);
}
#[test]
fn final_populated_row_checks_and_atomic_validation() {
    let mut lobby = host();
    lobby.action(Action::Kind(5), &names()).unwrap();
    assert!(lobby.start_enabled());
    for field in 0..3 {
        let mut invalid = lobby.clone();
        match field {
            0 => invalid.rows[5].player.ready = false,
            1 => invalid.rows[5].player.name.clear(),
            _ => invalid.rows[5].player.token = -1,
        }
        assert!(!invalid.start_enabled());
    }
    let before = lobby.clone();
    assert!(
        lobby
            .request(
                2,
                Request::Join {
                    name: vec![],
                    token: 0
                }
            )
            .is_err()
    );
    assert_eq!(lobby, before);
    assert!(lobby.action(Action::Start, &names()).is_err());
    assert_eq!(lobby, before);
}
#[test]
fn lobby_wire_round_trips_and_rejects_bad_rows() {
    use crate::network::{Frame, Message, codec};
    for message in [
        Message::Lobby(host()),
        Message::LobbyRequest(Request::Join {
            name: b"Client".to_vec(),
            token: 2,
        }),
        Message::LobbyRequest(Request::Edit {
            token: 3,
            ready: true,
        }),
        Message::LobbyDenied,
    ] {
        let frame = Frame {
            session: [2; 16],
            sequence: 0,
            message,
        };
        assert_eq!(codec::decode(&codec::encode(&frame).unwrap()), Ok(frame));
    }
    let mut lobby = host();
    lobby.rows[1].peer = Some(0);
    assert!(lobby.validate().is_err());
}
