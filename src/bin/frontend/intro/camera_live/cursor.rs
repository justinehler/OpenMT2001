//! Finite scalar part of 4391e0. All real UI/camera requesters share this value.
//! The existing host owns original images/hotspots; this is not an image queue.
use openmt2001::camera::primary::CursorRequest;

#[derive(Clone, Copy)]
pub struct Cursor {
    state: u32,
    priority: i32,
}

impl Cursor {
    /// Supply actual process state; neither a save nor camera mode supplies it.
    pub fn new(state: u32, priority: i32) -> Result<Self, String> {
        host_index(state)?;
        Ok(Self { state, priority })
    }
    pub fn select(
        &mut self,
        mut request: CursorRequest,
        force_default: bool,
    ) -> Result<(), String> {
        // Original 8b22d8 && 8b22dc && !86d675 override precedes priority.
        if force_default {
            request.state = 0;
        }
        if request.priority < self.priority {
            return Ok(());
        }
        host_index(request.state)?;
        self.priority = request.priority;
        self.state = request.state; // equality accepted: later request wins
        Ok(())
    }
    /// Original presentation369 lower_priority: once at the real presentation
    /// boundary, not once per camera catchup or arbitrary cursor query.
    pub fn presented(&mut self) {
        self.priority = self.priority.wrapping_sub(1);
    }
    pub fn host_index(&self) -> usize {
        // Constructor/select admit only states with an installed host image.
        host_index(self.state).unwrap_or(0)
    }
}

fn host_index(state: u32) -> Result<usize, String> {
    match state {
        0 => Ok(0),
        1 => Ok(3),
        2 => Ok(1),
        3 => Ok(2),
        4..=7 => Ok(state as usize),
        _ => Err(format!("Native cursor {state} has no installed host image")),
    }
}

/// The world pick (0x823168) as the pick-status pass reads it: the picked
/// descriptor's kind word (+0x30) and whether the related building (0x6175f8)
/// under the pointer is set. Kinds 1/2 are people and kind 4 is a city block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldPick {
    pub kind: u16,
    pub related: bool,
}

impl WorldPick {
    /// The engine's city pick returns a block, plus the building under the
    /// pointer when one is hit.
    pub fn from_hit(hit: crate::selection::Hit) -> Self {
        Self {
            kind: 4,
            related: hit.building.is_some(),
        }
    }
}

/// Cursor state the original pick-status pass (0x4c2f80) requests after its
/// default-arrow request. `choosing_footprint` is flag 0x54cd88 clear, which
/// the build form holds while the player drags a new building's footprint.
pub fn pick_status_state(pick: Option<WorldPick>, choosing_footprint: bool) -> u32 {
    let Some(pick) = pick else {
        return 0;
    };
    match pick.kind {
        1 | 2 => 6,
        3 | 9 => 5,
        4 if choosing_footprint => 7,
        4 if pick.related => 5,
        4 => 4,
        _ => 0,
    }
}

/// Host image for the live city's pick-status state when no control or camera
/// drag claims the cursor. `paused` is the selector's forced-default override
/// (0x8b22d8 pause in a running city), which turns every request into state 0.
pub fn pick_status_host_index(
    pick: Option<WorldPick>,
    choosing_footprint: bool,
    paused: bool,
) -> usize {
    if paused {
        return 0;
    }
    host_index(pick_status_state(pick, choosing_footprint)).unwrap_or(0)
}

/// The original pass's requests, in order: none while the right button is held,
/// otherwise the default arrow and then the picked state, both at priority 1.
pub fn pick_status_requests(
    right_held: bool,
    pick: Option<WorldPick>,
    choosing_footprint: bool,
) -> Vec<CursorRequest> {
    if right_held {
        return Vec::new();
    }
    let mut requests = vec![CursorRequest {
        state: 0,
        priority: 1,
    }];
    let state = pick_status_state(pick, choosing_footprint);
    if state != 0 {
        requests.push(CursorRequest { state, priority: 1 });
    }
    requests
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shown(cursor: &mut Cursor, requests: Vec<CursorRequest>) -> u32 {
        for request in requests {
            cursor.select(request, false).unwrap();
        }
        cursor.state
    }

    #[test]
    fn pick_status_maps_each_descriptor_to_the_original_state() {
        let pick = |kind, related| Some(WorldPick { kind, related });
        assert_eq!(pick_status_state(None, false), 0);
        for footprint in [false, true] {
            for related in [false, true] {
                assert_eq!(pick_status_state(pick(1, related), footprint), 6);
                assert_eq!(pick_status_state(pick(2, related), footprint), 6);
                assert_eq!(pick_status_state(pick(3, related), footprint), 5);
                assert_eq!(pick_status_state(pick(9, related), footprint), 5);
                assert_eq!(pick_status_state(pick(4, related), true), 7);
                for other in [0, 5, 6, 7, 8, 10, 11, 12] {
                    assert_eq!(pick_status_state(pick(other, related), footprint), 0);
                }
            }
        }
        assert_eq!(pick_status_state(pick(4, false), false), 4);
        assert_eq!(pick_status_state(pick(4, true), false), 5);
    }

    #[test]
    fn hit_without_a_building_is_a_block_and_with_one_is_a_building() {
        let hit = |building| crate::selection::Hit {
            category: 3,
            building,
        };
        assert_eq!(
            pick_status_state(Some(WorldPick::from_hit(hit(None))), false),
            4
        );
        assert_eq!(
            pick_status_state(Some(WorldPick::from_hit(hit(Some(0)))), false),
            5
        );
    }

    #[test]
    fn hover_states_select_their_own_host_images() {
        for state in 4..=7 {
            let cursor = Cursor::new(state, 0).unwrap();
            assert_eq!(cursor.host_index(), state as usize);
        }
        assert!(Cursor::new(8, 0).is_err());
    }

    #[test]
    fn right_button_keeps_the_current_cursor_and_release_resumes_hover() {
        let mut cursor = Cursor::new(0, 0).unwrap();
        let pick = Some(WorldPick {
            kind: 4,
            related: true,
        });
        assert_eq!(
            shown(&mut cursor, pick_status_requests(false, pick, false)),
            5
        );
        // A right drag's orbit cursor survives the pass while right is held.
        cursor
            .select(
                CursorRequest {
                    state: 2,
                    priority: 1,
                },
                false,
            )
            .unwrap();
        assert!(pick_status_requests(true, pick, false).is_empty());
        assert_eq!(
            shown(&mut cursor, pick_status_requests(true, pick, false)),
            2
        );
        cursor.presented();
        assert_eq!(
            shown(&mut cursor, pick_status_requests(false, None, false)),
            0
        );
    }

    #[test]
    fn live_city_host_images_follow_the_pick() {
        let pick = |kind, related| Some(WorldPick { kind, related });
        assert_eq!(pick_status_host_index(None, false, false), 0);
        assert_eq!(pick_status_host_index(pick(4, false), false, false), 4);
        assert_eq!(pick_status_host_index(pick(4, true), false, false), 5);
        assert_eq!(pick_status_host_index(pick(1, false), false, false), 6);
        assert_eq!(pick_status_host_index(pick(4, true), true, false), 7);
        assert_eq!(pick_status_host_index(pick(4, true), false, true), 0);
    }

    #[test]
    fn pause_forces_the_default_arrow_over_hover_states() {
        let mut cursor = Cursor::new(0, 0).unwrap();
        for request in pick_status_requests(
            false,
            Some(WorldPick {
                kind: 2,
                related: false,
            }),
            false,
        ) {
            cursor.select(request, true).unwrap();
        }
        assert_eq!(cursor.state, 0);
    }
}
