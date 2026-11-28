use super::*;

use super::test_player as player;

#[test]
fn stale_pre_six_create_and_missing_or_mismatched_release_are_tolerated() {
    let (mut player, mut output) = player();
    let first = player.apply(Operation::BeforeSixCreate).unwrap();
    assert_ne!(first, 0);
    let old_source = player.before_six.as_ref().unwrap().1.clone();
    let second = player.apply(Operation::BeforeSixCreate).unwrap();
    assert_ne!(second, first);
    // Stopping the stale source removes its samples from the mixer. Merely
    // overwriting our retained handle would keep both cues playing.
    let samples: Vec<_> = output.by_ref().take(4096).collect();
    assert!(
        samples[2048..]
            .iter()
            .all(|sample| (*sample - 0.5).abs() < 1e-6)
    );
    assert!(old_source.empty());
    player.apply(Operation::BeforeSixRelease(first)).unwrap();
    assert_eq!(player.before_six.as_ref().unwrap().0, second);
    assert!(player.warned_before_six_release);
    player.apply(Operation::BeforeSixRelease(second)).unwrap();
    assert!(player.before_six.is_none());
    player.apply(Operation::BeforeSixRelease(second)).unwrap();
    player.apply(Operation::BeforeSixRelease(0)).unwrap();
    // Exhausting a presentation token must not fail the simulation either.
    player.next_handle = u32::MAX;
    assert_eq!(player.apply(Operation::BeforeSixCreate).unwrap(), u32::MAX);
    assert_eq!(player.apply(Operation::BeforeSixCreate).unwrap(), 1);
}
