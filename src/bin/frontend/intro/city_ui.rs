//! The city HUD belongs to the primary group; statistics and Options own the
//! secondary group in mode4 (49dcb0/4b5370). Keep the retained city view intact.
use crate::{live_host::StatsAction, stats_panel};

#[derive(Clone, Copy)]
pub(crate) struct Layers {
    options: bool,
    pub statistics: bool,
}

pub(crate) enum MouseRoute {
    City,
    Statistics(Option<StatsAction>),
}

impl Layers {
    pub(crate) fn new(
        options: bool,
        requested: bool,
        snapshot: Option<&stats_panel::Snapshot>,
    ) -> Self {
        Self {
            options,
            statistics: requested || snapshot.is_some(),
        }
    }

    pub(crate) fn hud_visible(self) -> bool {
        !self.options && !self.statistics
    }

    pub(crate) fn mouse(
        self,
        panel: &mut stats_panel::View,
        assets: &stats_panel::Assets,
        snapshot: Option<&stats_panel::Snapshot>,
        point: Option<[f32; 2]>,
        left_button: bool,
        pressed: bool,
    ) -> MouseRoute {
        if !self.statistics {
            return MouseRoute::City;
        }
        // Capture even outside the graph, and while its worker reply is pending.
        // Hidden primary-group widgets must never see either mouse edge.
        MouseRoute::Statistics(if left_button {
            snapshot.and_then(|snapshot| panel.pointer(assets, point, pressed, snapshot))
        } else {
            None
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statistics_captures_hidden_hud_until_ok_restores_the_city() {
        let assets = stats_panel::Assets::input_test_assets();
        let mut panel = stats_panel::View::default();
        let mut layers = Layers::new(false, false, None);
        assert!(layers.hud_visible());

        // The graph button's request hides/captures immediately, before the
        // asynchronous worker has published the graph's snapshot.
        layers = Layers::new(false, true, None);
        assert!(!layers.hud_visible());
        assert!(matches!(
            layers.mouse(&mut panel, &assets, None, Some([580., 460.]), true, true),
            MouseRoute::Statistics(None)
        ));
        let snapshot = stats_panel::Snapshot {
            state: stats_panel::State::show(0, true, 1).unwrap(),
            players: vec![stats_panel::Player {
                character: 0,
                name: b"Player".to_vec(),
            }],
            history: vec![], // Input never reads graph history or uses a GPU.
            years_per_day: 5,
        };
        layers = Layers::new(false, false, Some(&snapshot));
        assert!(!layers.hud_visible());
        // Block/detail, clock/money, miniature navigator/compass/graph/speed,
        // and comparison coordinates cannot fall through to the city handler.
        for point in [
            [20., 50.],
            [600., 20.],
            [580., 400.],
            [580., 460.],
            [20., 450.],
        ] {
            for left_button in [true, false] {
                for pressed in [true, false] {
                    assert!(matches!(
                        layers.mouse(
                            &mut panel,
                            &assets,
                            Some(&snapshot),
                            Some(point),
                            left_button,
                            pressed,
                        ),
                        MouseRoute::Statistics(None)
                    ));
                }
            }
        }
        // Exercise the real graph widget's press/release action through the
        // same routing function as Inspection::statistics_mouse.
        let mut closed = false;
        for pressed in [true, false] {
            let route = layers.mouse(
                &mut panel,
                &assets,
                Some(&snapshot),
                Some([320., 415.]),
                true,
                pressed,
            );
            match route {
                MouseRoute::Statistics(Some(StatsAction::Close)) if !pressed => closed = true,
                MouseRoute::Statistics(None) if pressed => {}
                _ => panic!("Ok must close only on release"),
            }
            assert!(!layers.hud_visible(), "retain capture until worker closes");
        }
        assert!(closed);
        layers = Layers::new(false, false, None); // worker's closed snapshot
        assert!(layers.hud_visible());
        assert!(matches!(
            layers.mouse(&mut panel, &assets, None, Some([580., 460.]), true, true),
            MouseRoute::City
        ));
    }
}
