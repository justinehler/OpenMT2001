use super::*;
use openmt2001::{
    level_catalog::{Catalog, Labels, Registration},
    outcome_notice::Sequence,
};

struct Scratch(std::path::PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "openmt-final-statistics-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn catalog(id: i32) -> Catalog {
    let mut catalog = Catalog::default();
    for id in [id, id + 1] {
        catalog
            .register(
                Registration {
                    id,
                    directory: b"test".to_vec(),
                    labels: Labels::Literal {
                        title: vec![],
                        description: vec![],
                    },
                    checksum: 0,
                },
                false,
                true,
            )
            .unwrap();
    }
    catalog.unlock_first();
    catalog
}
fn pending(sequence: Sequence) -> Host {
    let mut host = Host::new([0.; 3]);
    host.game_active = true;
    host.result_request = Some(ResultRequest {
        sequence,
        characters: (0, 1),
        music_address: 0,
    });
    host
}

#[test]
fn final_statistics_result_close_menu_commits_progression_once() {
    let scratch = Scratch::new();
    for (sequence, end, won) in [
        (Sequence::Win, 1, true),
        (Sequence::Lose, 2, false),
        (Sequence::Complete, 1, true),
    ] {
        for (id, target) in [(200003, ResultMenu::Scenarios), (500000, ResultMenu::Main)] {
            for objective in 0..=6 {
                let mut catalog = catalog(id);
                let mut host = pending(sequence);
                let path = scratch.0.join(format!("{sequence:?}-{id}-{objective}.lua"));
                let stats = crate::stats_panel::State::show(objective, false, 2).unwrap();
                assert_eq!(host.result_review(&catalog), None);
                assert!(
                    host.commit_result_review(&mut catalog, Some(&path), stats.clone(), None)
                        .is_err()
                );
                assert_eq!(host.result_review(&catalog), None);
                assert_eq!(catalog.levels[0].attempts(), 0);
                assert!(!path.exists());
                assert_eq!(
                    host.commit_result_review(&mut catalog, Some(&path), stats.clone(), Some(end))
                        .unwrap(),
                    sequence
                );
                assert_eq!(host.result_review(&catalog), Some(ResultReview::Statistics));
                assert!(host.result_request.is_none());
                assert!(!host.game_active && !host.running);
                host.set_running(true);
                assert!(
                    !host.running,
                    "a queued resume cannot restart a finished game"
                );
                let initial_mode = match objective {
                    1 | 2 => 5,
                    3 => 4,
                    4 | 5 => 1,
                    6 => 0,
                    _ => 2,
                };
                assert_eq!(host.stats.as_ref().unwrap().mode, initial_mode);
                let mut modes = std::collections::BTreeSet::new();
                for _ in 0..6 {
                    let state = host.stats.as_mut().unwrap();
                    modes.insert(state.mode);
                    state.cycle(true, objective, host.game_active);
                }
                assert_eq!(modes.len(), if matches!(objective, 1 | 2) { 6 } else { 5 });
                let saved = std::fs::read(&path).unwrap();
                assert!(
                    host.commit_result_review(&mut catalog, Some(&path), stats.clone(), Some(end))
                        .is_err()
                );
                assert_eq!(host.result_review(&catalog), Some(ResultReview::Statistics));
                // Game.End already owns reason0. Closing a stopped graph must
                // not release it or change any timer/pause state.
                let mut pause = openmt2001::pause::State::default();
                pause.bytes[0] = 1;
                let before = pause.bytes;
                for _ in 0..2 {
                    host.close_statistics(&mut pause, &mut []).unwrap();
                    assert_eq!(pause.bytes, before);
                    assert_eq!(
                        host.result_review(&catalog),
                        Some(ResultReview::Menu(target))
                    );
                }
                assert!(
                    host.commit_result_review(&mut catalog, Some(&path), stats, Some(end))
                        .is_err()
                );
                assert_eq!(std::fs::read(&path).unwrap(), saved);
                let mut reopened = catalog.clone();
                crate::campaign_store::load_progress(&mut reopened, &path).unwrap();
                assert_eq!(reopened.levels[0].attempts(), 1);
                assert_eq!(reopened.levels[0].completed()[0], won);
                if target == ResultMenu::Scenarios {
                    assert_eq!(reopened.levels[1].unlocked(), won);
                }
            }
        }
    }
}

#[test]
fn final_statistics_persistence_retry_does_not_repeat_progression() {
    let scratch = Scratch::new();
    let parent = scratch.0.join("blocked");
    std::fs::write(&parent, b"blocked parent").unwrap();
    let path = parent.join("profile.lua");
    let mut catalog = catalog(200003);
    let mut host = pending(Sequence::Win);
    let stats = crate::stats_panel::State::show(1, false, 1).unwrap();
    assert!(
        host.commit_result_review(&mut catalog, Some(&path), stats.clone(), Some(1))
            .is_err()
    );
    assert!(host.result_applied);
    assert!(!host.result_completed);
    assert_eq!(host.result_review(&catalog), None);
    assert_eq!(catalog.levels[0].attempts(), 1);
    std::fs::remove_file(&parent).unwrap();
    std::fs::create_dir(&parent).unwrap();
    host.commit_result_review(&mut catalog, Some(&path), stats, Some(1))
        .unwrap();
    assert_eq!(catalog.levels[0].attempts(), 1);
    assert_eq!(host.result_review(&catalog), Some(ResultReview::Statistics));
}
