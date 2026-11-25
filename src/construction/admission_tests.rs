//! Literal expectations from4517f2/445b10/445b90/451894, not a second
//!implementation oracle. Synthetic rows exercise transport; no quote is run.
use super::*;
use crate::{player_manager::{Player, Auxiliary, put, word}, player_messages as pm};

fn player(slot: u8, ai: bool) -> Player {
    let mut p = Player { reference: 0x10010000 + u32::from(slot) * 0x10000,
        bytes: vec![0; 0x1e50], auxiliaries: vec![] };
    p.bytes[0x10] = 1;
    p.bytes[0x1bc] = slot;
    put(&mut p.bytes, 0x154, u32::from(ai));
    p
}
fn command() -> Command {
    Command { category: 3, rectangle: [0, 2, 1, 3], height: 4,
        kind: 0, business: 7, variation: 9, quality: 2, owner: 2,
        //Distinct stored bits detect repricing/field permutation. These are
        //payload witnesses, not a gameplay quote or affordability expectation.
        components: [f32::from_bits(0x3f800001), f32::from_bits(0x40000001),
            f32::from_bits(0x40400001), f32::from_bits(0x40800001)] }
}
fn routing() -> pm::Routing {
    pm::Routing { current_player: 0x10010000, network: [false; 2], prior: [0xa5; 48] }
}

#[test]
fn accepted_rectangle_reserves_before_queue_and_blocks_second_packet() {
    let mut categories = [[0; 380]; 52];
    let mut players = vec![player(0, false), player(1, true), player(2, false)];
    let mut arena = pm::Arena { next: 0x20000000 };
    let first = command();
    let packet = first.packet();
    assert!(admit(&first, &mut categories, &mut players, &mut arena, 3, routing(),
        |_, _| Err("accepted construction emitted an external message".into())).unwrap());
    //445b90 uses x*7+y, inclusive [0,2]..[1,3].
    let mut expected = [0; 49];
    for at in [2, 3, 9, 10] { expected[at] = 4; }
    assert_eq!(categories[3][0x9c..0xcd], expected);
    assert_eq!(first.packet(), packet, "admission cannot reprice the payload");
    assert_eq!(arena.next, 0x20000000);
    let mut queue = crate::live_commands::Queue::default();
    queue.submit_with_delay(crate::live_commands::Command::Build(first.clone()), 100, true, 6,
        |_| Err("accepted packet reached consumer before due".into())).unwrap();
    let rows = queue.snapshot().unwrap();
    assert_eq!((rows[0].1, rows[0].2, rows[0].3), (131, 131, false));
    let crate::live_commands::Command::Build(queued) = rows[0].4 else { panic!("wrong command") };
    assert_eq!(queued.packet(), packet);
    //Same authoritative grid rejects the second pending command. Recipient
    //owner2 means slot1: adjacent human slots must not receive the packet.
    assert!(!admit(&first, &mut categories, &mut players, &mut arena, 3, routing(),
        |_, _| Err("AI rejection must enter its local inbox".into())).unwrap());
    assert_eq!(queue.snapshot().unwrap().len(), 1);
    assert_eq!(categories[3][0x9c..0xcd], expected);
    assert_eq!(arena.next, 0x20000030);
    assert_eq!(word(&players[0].bytes, 0x214), 0);
    assert_eq!(word(&players[2].bytes, 0x214), 0);
    assert_eq!(word(&players[1].bytes, 0x214), 1);
    let message = &players[1].auxiliaries[0];
    assert_eq!(message.reference, 0x20000000);
    assert_eq!(word(&players[1].bytes, 0x218), message.reference);
    assert_eq!(word(&message.bytes, 4), 0x30);
    assert_eq!(&message.bytes[0x14..0x16], &[0xff, 1]);
    assert_eq!([0x18, 0x1c, 0x20, 0x24, 0x28, 0x2c].map(|at| word(&message.bytes, at)),
        [3, 7, 0, 0, u32::MAX, 0]);
    assert_eq!(message.bytes[0x11], 0xa5, "real message prior bytes survive");
}

#[test]
fn ai_producer_packet_uses_real_split_owner_table_and_inbox_tail() {
    let mut categories = [[0; 380]; 52];
    categories[3][0x9c + 2] = 9;
    let mut players = vec![player(0, true), player(1, true), player(2, true)];
    //An existing inbox node tests append/link mutation, not a test-only notice.
    let first_ref = 0x21000000;
    put(&mut players[1].bytes, 0x214, 1);
    put(&mut players[1].bytes, 0x218, first_ref);
    players[1].auxiliaries.push(Auxiliary { reference: first_ref, bytes: [0; 48] });
    let mut arena = pm::Arena { next: 0x20000000 };
    let args = [3, 0, 2, 2, 2, 4, 9, 0, 2, 0, 7];
    let (before, tail) = players.split_at_mut(1);
    let (current, after) = tail.split_first_mut().unwrap();
    //Actual48ac20 descriptor producer, including its one-based owner byte;
    //the quote is outside this admission-only test's numerical scope.
    let packet = crate::ai_actions::construction::descriptor(current, args, command().components, [0; 60]);
    let command = Command::from_packet(&packet).unwrap();
    assert_eq!(command.rectangle, [0, 2, 1, 3]);
    assert_eq!(command.owner, 2);
    assert!(!admit(&command, &mut categories,
        &mut pm::SplitPlayers { before, current, after }, &mut arena, 3, routing(),
        |_, _| Err("AI message escaped authoritative split inbox".into())).unwrap());
    assert_eq!(word(&players[1].bytes, 0x214), 2);
    assert_eq!(word(&players[1].auxiliaries[0].bytes, 0x2c), 0x20000000);
    assert_eq!(word(&players[1].auxiliaries[1].bytes, 0x1c), 7);
    assert!(players[0].auxiliaries.is_empty());
    assert!(players[2].auxiliaries.is_empty());
}

#[test]
fn rectangle_rejection_is_sender_success_without_enqueue_or_human_notice() {
    for rectangle in [[1, 1, 2, 2], [-1, 0, 1, 1], [0, 0, 7, 1], [2, 0, 1, 1], [0, 2, 1, 1]] {
        let mut categories = [[0; 380]; 52];
        let mut players = vec![player(0, false)];
        let mut arena = pm::Arena { next: 0x20000000 };
        let mut command = command(); command.owner = 1; command.rectangle = rectangle;
        let admitted = admit(&command, &mut categories, &mut players, &mut arena, 1, routing(),
            |_, _| Err("human rejection sent a message".into())).unwrap();
        assert!(!admitted, "445b10 literal invalid rectangle {rectangle:?}");
        assert_eq!(categories[3][0x9c..0xcd], [0; 49]);
        assert!(players[0].auxiliaries.is_empty());
        assert_eq!(arena.next, 0x20000000);
    }
}

#[test]
fn inactive_ai_and_signed_owner_failures_keep_native_mutation_order() {
    let mut categories = [[0; 380]; 52]; categories[3][0x9c + 2] = 9;
    let mut players = vec![player(0, false), player(1, true)];
    players[1].bytes[0x10] = 0;
    let mut arena = pm::Arena { next: 0x20000000 };
    assert!(!admit(&command(), &mut categories, &mut players, &mut arena, 2, routing(),
        |_, _| Err("inactive AI sent message".into())).unwrap());
    assert_eq!(arena.next, 0x20000000);
    assert!(players[1].auxiliaries.is_empty());
    for owner in [0, -1, -128, 3] {
        let mut invalid = command(); invalid.owner = owner;
        assert!(admit(&invalid, &mut categories, &mut players, &mut arena, 2, routing(),
            |_, _| Err("invalid owner sent message".into())).is_err());
    }
    players[1].bytes[0x10] = 1;
    players[1].auxiliaries.push(Auxiliary { reference: arena.next, bytes: [0; 48] });
    assert!(admit(&command(), &mut categories, &mut players, &mut arena, 2, routing(),
        |_, _| Err("colliding inbox sent message".into())).unwrap_err().contains("collides"));
    assert_eq!(arena.next, 0x20000030, "allocation precedes collision validation");
    assert_eq!(word(&players[1].bytes, 0x214), 0, "count follows successful allocation");
    assert_eq!(categories[3][0x9c + 2], 9, "reject path has no footprint store");
}

#[test]
fn reservation_survives_a_later_queue_consumer_error() {
    let mut categories = [[0; 380]; 52];
    let mut players = vec![player(0, false), player(1, true)];
    let mut arena = pm::Arena { next: 0x20000000 };
    let command = command();
    assert!(admit(&command, &mut categories, &mut players, &mut arena, 2, routing(),
        |_, _| Err("accepted construction sent message".into())).unwrap());
    let mut queue = crate::live_commands::Queue::default();
    assert!(queue.submit_with_delay(crate::live_commands::Command::Build(command), 100, false, 0,
        |_| Err("injected synchronous consumer failure".into())).is_err());
    assert_eq!(categories[3][0x9c + 2], 4, "45184e is committed before450a30");
}

#[test]
fn saved_pending_queue_restore_preserves_packet_and_deadline() {
    let mut queue = crate::live_commands::Queue::default();
    let command = command(); let packet = command.packet();
    //Transport-only deadline/payload check. The connected cold Session case
    //in original_construction_lifecycle checks the actual no-readmission path.
    queue.restore_queued(crate::live_commands::Command::Build(command),
        crate::scheduler::ClockDomain::Total, 125, 125, false);
    let mut applied = Vec::new();
    queue.drain(&mut [124, 999], |_| Err("saved command applied early".into())).unwrap();
    queue.drain(&mut [125, 999], |c| {
        let crate::live_commands::Command::Build(c) = c else { panic!("wrong command") };
        applied.push(c.packet()); Ok(())
    }).unwrap();
    assert_eq!(applied, [packet]);
    assert!(queue.snapshot().unwrap().is_empty());
}

#[test]
#[ignore = "requires original PE for the real AI inbox consumer context"]
fn rejected_build_message30_reaches_real_count_removal_consumer() {
    let bytes = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
    let image = crate::pe::PeImage::parse(&bytes).unwrap();
    let mut categories = [[0; 380]; 52]; categories[3][0x9c + 2] = 9;
    let mut players = vec![player(0, false), player(1, true)];
    let scratch = 0x21000000;
    put(&mut players[1].bytes, 0x210, scratch);
    players[1].auxiliaries.push(Auxiliary { reference:scratch, bytes:[0;48] });
    let mut arena = pm::Arena { next:0x20000000 };
    assert!(!admit(&command(), &mut categories, &mut players, &mut arena, 2, routing(),
        |_, _| Err("AI rejection escaped its inbox".into())).unwrap());
    let mut counts = [0; 52*45]; counts[3*45+7] = 1;
    let mut profiles = [[0;0x650];10];
    let mut context = crate::ai_messages::Context { image:&image, profiles:&mut profiles,
        buildings:&[], auction:None };
    let mut consumed = 0;
    crate::ai_messages::consume(&mut players[1], &mut context, |call,_,_| {
        assert_eq!((call.address,call.object,call.count),(0x472620,0,2));
        assert_eq!(call.arguments,[3,7,0]);
        crate::ai_business::remove_category_count(&mut counts,call.arguments[0],call.arguments[1])?;
        consumed += 1;
        Ok(crate::ai_messages::Reply::default())
    }).unwrap();
    assert_eq!(consumed,1);
    assert_eq!(counts[3*45+7],0,"native4692bc ->472620 compensates category/type count");
    assert_eq!(word(&players[1].bytes,0x214),0);
    assert_eq!(word(&players[1].bytes,0x218),0);
    assert_eq!(word(&players[1].bytes,0x20c),scratch);
    assert_eq!(arena.next,0x20000030);
}

#[test]
fn accepted_height_is_the_literal_payload_byte_including_zero_and_negative() {
    for height in [0,1,-1,-128] {
        let mut categories = [[0;380];52];
        let mut players = Vec::<Player>::new();
        let mut arena = pm::Arena { next:0x20000000 };
        let mut command = command(); command.height=height;
        //No owner-table access on acceptance, as in451822..451853.
        assert!(admit(&command,&mut categories,&mut players,&mut arena,0,routing(),
            |_,_|Err("accepted packet sent a message".into())).unwrap());
        assert_eq!(categories[3][0x9c+2],height as u8);
        assert_eq!(arena.next,0x20000000);
    }
}
