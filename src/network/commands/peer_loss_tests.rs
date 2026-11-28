//! Peer loss during play over the real loopback workers (0x4e2740 -> 0x450670).
//! The departed client's final request is admitted first; the authority then
//! schedules status `(player, 3)` and removal for the same Total+25 on itself
//! and every remaining client.
use super::*;
use std::{
    thread,
    time::{Duration, Instant},
};

const WAIT: Duration = Duration::from_secs(60);
/// Below the first ceiling (25), so no grant closes the pumping tick.
const TICK: u32 = 10;

/// A session with a third human row (compact owner 3) and Total at `TICK`.
fn three_humans(image: &crate::pe::PeImage<'_>, board: &crate::board::Board) -> TestSession {
    let mut session = TestSession::new(image, board);
    let mut player = player_manager::Player {
        reference: 0x3000,
        bytes: session.roster.manager.players[0].bytes.clone(),
        auxiliaries: vec![],
    };
    player.bytes[0x1bc] = 2;
    session.roster.manager.players.push(player);
    session
        .roster
        .collect_accounts(&mut session.world.city)
        .unwrap();
    session.world.clock.timing[0] = TICK;
    session
}

fn pump(
    adapter: &mut Adapter,
    worker: &crate::network::lobby::worker::Handle,
    session: &mut TestSession,
) {
    adapter
        .pump(
            worker,
            TICK,
            &mut session.queue,
            &mut session.world.city.categories,
            Targets::default(),
            &mut session.world.city.pause,
            &mut session.tasks,
        )
        .unwrap();
    adapter
        .remove_departed(
            worker,
            &session.roster.manager.players,
            &mut session.queue,
            &mut session.world.city.categories,
            TICK,
        )
        .unwrap();
}

/// (kind, due) of each queued command, in queue order.
fn queued(session: &TestSession) -> Vec<(String, u32)> {
    session
        .queue
        .snapshot()
        .unwrap()
        .iter()
        .map(|(_, due, _, _, command)| {
            let name = match command {
                live_commands::Command::PropertyTransfer {
                    category,
                    owner,
                    amount,
                } => format!("transfer {category} {owner} {amount}"),
                live_commands::Command::OutcomeNotice { player, reason } => {
                    format!("status {player} {reason}")
                }
                live_commands::Command::Eliminate { slot } => format!("removal {slot}"),
                _ => "other".into(),
            };
            (name, *due)
        })
        .collect()
}

#[test]
fn departed_client_request_then_status_and_removal_apply_identically_on_host_and_remaining_client()
{
    let bytes = image_bytes();
    let image = crate::pe::PeImage::parse(&bytes).unwrap();
    let board = crate::board::Board::from_v14(&image).unwrap();
    let mut left = three_humans(&image, &board);
    let mut right = three_humans(&image, &board);
    let (host, mut clients) = crate::network::lobby::worker::tests::playing_peers(2);
    let (remaining, remaining_peer) = clients.pop().unwrap();
    let (departing, departing_peer) = clients.pop().unwrap();
    let hb = host.bind_world(1).unwrap();
    let db = departing.bind_world(2).unwrap();
    let rb = remaining.bind_world(3).unwrap();
    // Compact owner 1 is the host, owner 2 the departing client, owner 3 the other.
    let owners = vec![Some(0), Some(departing_peer), Some(remaining_peer)];
    let mut authority = Adapter::new(hb, true, 0, owners.clone()).unwrap();
    let mut replica = Adapter::new(rb, false, remaining_peer, owners).unwrap();

    // The departing client's last request, followed on the same ordered stream
    // by a chat line: once the host shows the line, the request has arrived.
    departing
        .send_gameplay(
            db,
            None,
            Message::CommandRequest(Request::transfer(
                0,
                Transfer {
                    category: 7,
                    owner: 2,
                    amount: 500,
                    due: 0,
                },
            )),
        )
        .unwrap();
    assert!(departing.submit_chat(b"bye".to_vec(), None).unwrap());
    let limit = Instant::now() + WAIT;
    while !host
        .chat_view()
        .is_some_and(|view| view.lines.iter().any(|line| line.text.ends_with(b"bye")))
    {
        assert!(Instant::now() < limit, "final chat line did not arrive");
        thread::sleep(Duration::from_millis(5));
    }
    drop(departing);
    // Wait for the host transport to see the end of that connection before
    // the Session consumes anything: the request is now a departed peer's.
    let barrier = Message::Barrier {
        sender: 0,
        complete: 0,
    };
    while host
        .send_synchronization(hb, Some(departing_peer), barrier.clone())
        .unwrap()
    {
        assert!(Instant::now() < limit, "connection end was not observed");
        thread::sleep(Duration::from_millis(5));
    }

    let expected = vec![
        ("transfer 7 2 500".to_string(), TICK + 25),
        ("status 1 3".to_string(), TICK + 25),
        ("removal 1".to_string(), TICK + 25),
    ];
    while queued(&left) != expected || queued(&right) != expected {
        pump(&mut authority, &host, &mut left);
        pump(&mut replica, &remaining, &mut right);
        assert!(
            queued(&left).len() <= 3 && queued(&right).len() <= 3,
            "{:?} {:?}",
            queued(&left),
            queued(&right)
        );
        assert!(
            Instant::now() < limit,
            "{:?} {:?}",
            queued(&left),
            queued(&right)
        );
        thread::sleep(Duration::from_millis(5));
    }
    // A repeated loss of the same connection is ignored.
    let again = Connection {
        attempt: hb.attempt(),
        slot: departing_peer,
        identity: 0,
    };
    authority.lose(hb, again).unwrap();
    for _ in 0..5 {
        pump(&mut authority, &host, &mut left);
        pump(&mut replica, &remaining, &mut right);
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(queued(&left), expected);
    assert_eq!(queued(&right), expected);
    // A client cannot request a status or removal itself.
    assert_eq!(
        replica.request(
            remaining.gameplay_owner(),
            Request::player_removal(0, 2),
            TICK,
            Targets::default()
        ),
        Err(Error::Peer)
    );

    for session in [&mut left, &mut right] {
        while session.world.clock.timing[0] < TICK + 24 {
            session.tick(&image, &board);
        }
        assert_eq!(session.world.categories[7][5], 0);
        assert_eq!(session.roster.manager.players[1].bytes[0x10], 1);
        session.tick(&image, &board);
        assert_eq!(session.world.clock.timing[0], TICK + 25);
        assert!(session.queue.snapshot().unwrap().is_empty());
        // The request applied first, then removal released that property.
        assert_eq!(session.postings.len(), 1);
        assert_eq!(session.world.ledgers.players[1].cash, 9500.);
        assert_eq!(session.world.categories[7][5], 0);
        let departed = &session.roster.manager.players[1].bytes;
        assert_eq!(departed[0x10], 0, "departed player stays inactive");
        assert_eq!(player_manager::word(departed, 0x154), 2);
        assert_eq!(departed[0x178 + 7], 0);
        // The others keep playing.
        assert_eq!(session.roster.manager.players[0].bytes[0x10], 1);
        assert_eq!(session.roster.manager.players[2].bytes[0x10], 1);
    }
    assert_eq!(left.world.city.categories, right.world.city.categories);
    assert_eq!(
        left.roster
            .manager
            .players
            .iter()
            .map(|p| &p.bytes)
            .collect::<Vec<_>>(),
        right
            .roster
            .manager
            .players
            .iter()
            .map(|p| &p.bytes)
            .collect::<Vec<_>>()
    );
    assert_eq!(left.roster.manager.bytes, right.roster.manager.bytes);
}

/// An already eliminated player whose connection ends gets no second status.
#[test]
fn loss_of_an_inactive_player_schedules_nothing() {
    let bytes = image_bytes();
    let image = crate::pe::PeImage::parse(&bytes).unwrap();
    let board = crate::board::Board::from_v14(&image).unwrap();
    let mut left = TestSession::new(&image, &board);
    left.world.clock.timing[0] = TICK;
    left.roster.manager.players[1].bytes[0x10] = 0;
    let (host, client, peer) = crate::network::lobby::worker::tests::playing_pair(false);
    let hb = host.bind_world(1).unwrap();
    client.bind_world(2).unwrap();
    let mut authority = Adapter::new(hb, true, 0, vec![Some(0), Some(peer)]).unwrap();
    drop(client);
    let limit = Instant::now() + WAIT;
    while !authority.departed[usize::from(peer)] {
        authority
            .pump(
                &host,
                TICK,
                &mut left.queue,
                &mut left.world.city.categories,
                Targets::default(),
                &mut left.world.city.pause,
                &mut left.tasks,
            )
            .unwrap();
        assert!(Instant::now() < limit, "connection end was not observed");
        thread::sleep(Duration::from_millis(5));
    }
    authority
        .remove_departed(
            &host,
            &left.roster.manager.players,
            &mut left.queue,
            &mut left.world.city.categories,
            TICK,
        )
        .unwrap();
    assert!(authority.departures.is_empty());
    assert!(left.queue.snapshot().unwrap().is_empty());
}
