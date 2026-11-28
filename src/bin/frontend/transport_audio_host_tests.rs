//! Transport-audio tests that drive the live Host. They build with the game
//! binary; transport_audio itself is in crates/frontend (#99).
use crate::transport_audio::test_city_inputs as city_inputs;
use crate::transport_audio::test_owner as owner;
use crate::transport_audio::*;

#[test]
fn city_frame_keeps_construction_during_pause_and_repositions_before_resume() {
    let (mut player, _, mut output) = owner();
    let (sender, receiver) = std::sync::mpsc::channel();
    let mut host = crate::live_host::Host::new([0., 40., -300.]);
    host.transport_audio = Some(sender.clone());
    host.set_running(true);
    submit(
        &Some(sender.clone()),
        Operation::City(Box::new(city_inputs(101))),
    )
    .unwrap();
    player.drain_requests(Some(&receiver));
    assert!(player.construction[0].is_none()); // Receiving inputs does not service a frame.
    player.service_frame([0., 40., -300.]);
    let sink = player.construction[0].as_ref().unwrap();
    let retained = &**sink as *const rodio::Sink;
    let near_gain = sink.volume();
    assert!(near_gain > 0.);
    assert!(player.coast.as_ref().unwrap().volume() > 0.);
    assert!(player.resident_crowd.as_ref().unwrap().volume() > 0.);
    assert!(player.block_sinks[2].as_ref().unwrap().volume() > 0.);
    let pcm: Vec<_> = output.by_ref().take(4096).collect();
    assert!(pcm[2048..].iter().all(|sample| *sample > 0.));
    let position = sink.get_pos();

    host.set_running(false);
    assert!(matches!(
        receiver.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Empty)
    ));
    for camera in [[200., 40., -300.], [400., 40., 0.], [0., 40., -300.]] {
        player.drain_requests(Some(&receiver));
        player.service_frame(camera);
        let sink = player.construction[0].as_ref().unwrap();
        assert_eq!(&**sink as *const rodio::Sink, retained);
        if camera[0] == 200. {
            assert!(sink.volume() > 0. && sink.volume() < near_gain);
        } else if camera[0] == 400. {
            assert_eq!(sink.volume(), 0.);
            assert_eq!(player.coast.as_ref().unwrap().volume(), 0.);
            assert!(player.resident_crowd.is_none());
            assert!(player.block_sinks[2].is_none());
        } else {
            assert_eq!(sink.volume(), near_gain);
            assert!(player.coast.as_ref().unwrap().volume() > 0.);
        }
        output.by_ref().take(4096).for_each(drop);
        assert_eq!(player.city.as_ref().unwrap().tick, 101);
    }
    assert!(player.construction[0].as_ref().unwrap().get_pos() > position);
    host.set_running(true);
    let mut finished = city_inputs(102);
    finished.construction = [[None; 2]; 4];
    submit(&Some(sender), Operation::City(Box::new(finished))).unwrap();
    player.drain_requests(Some(&receiver));
    assert!(player.construction[0].is_some());
    player.service_frame([0., 40., -300.]);
    assert!(player.construction[0].is_none());
    player.apply(Operation::Reset);
    player.service_frame([0., 40., -300.]);
    assert!(player.city.is_none() && player.coast.is_none());
}
