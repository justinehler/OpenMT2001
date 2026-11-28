//! Locator tests that need intro's focus state. They build with the game
//! binary; the Locator itself is in crates/frontend (#99).
use crate::locator::refresh_ui_tests::fresh;
use crate::locator::*;
#[allow(unused_imports)]
use crate::ui::{self, Surface};
#[allow(unused_imports)]
use openmt2001::{
    archive::ArchiveIndex,
    business_pool::BusinessCatalog,
    pe::PeImage,
    render::{PreparedScene, Renderer},
    render_scene::Scene,
    texture::TextureSet,
};
#[allow(unused_imports)]
use std::{fs, path::Path};

#[test]
fn city_tab_hover_help_uses_regions_and_shared_dwell() {
    use crate::intro::camera_live::focus::{ChildKey, Focus};
    let mut state = fresh();
    state.city_info = true;
    state.open = true;
    state.prestige = Some(crate::city_prestige::Prestige {
        rows: vec![],
        values: [0.; 52],
        overlay_categories: vec![],
    });
    state.rights = Some(crate::city_rights::Rights {
        categories: [crate::city_rights::Category {
            mode: 0,
            owner: 0,
            remaining: 0,
        }; 52],
        owner_number: 1,
    });
    state.ownership = Some(crate::city_ownership::Ownership {
        badges: [crate::ownership_badges::Badge {
            character: None,
            remaining: 0,
        }; 52],
        groups: vec![],
    });
    let layout = ui::mpp_modes([30., 30., 190., 245.], [25, 25]).unwrap();
    let target = |state: &Locator, point| state.city_mode_at(&layout, point).and_then(icon_tooltip);
    let mut focus = Focus::default();
    let mut fallback = crate::tooltip::IconDwell::default();
    for (index, id) in [1150, 1151, 1152, 1153].into_iter().enumerate() {
        let [l, t, r, b] = layout.buttons[index];
        let point = [(l + r) / 2., (t + b) / 2.];
        assert_eq!(target(&state, point), Some(id));
        for tick in 1..=26 {
            //Repeated presentation of the same widget preserves dwell.
            focus
                .show_child(ChildKey::Locator(id), target(&state, point).unwrap(), false)
                .unwrap();
            focus.advance();
            let expected = (tick > 25).then_some(id);
            assert_eq!(focus.caption(), expected);
            assert_eq!(fallback.advance(target(&state, point)), expected);
        }
        for point in [[r + 1., (t + b) / 2.], [(l + r) / 2., b + 1.]] {
            assert_eq!(target(&state, point), None);
        }
        focus.clear_child();
        assert_eq!(focus.caption(), None);
        assert_eq!(fallback.advance(None), None);
        state.open = false;
        assert_eq!(target(&state, point), None);
        state.open = true;
        state.city_info = false;
        assert_eq!(target(&state, point), None);
        state.city_info = true;
    }
    //A direct move between tabs replaces help and starts a fresh dwell.
    focus
        .show_child(ChildKey::Locator(1150), 1150, false)
        .unwrap();
    for _ in 0..26 {
        focus.advance();
    }
    assert_eq!(focus.caption(), Some(1150));
    focus
        .show_child(ChildKey::Locator(1151), 1151, false)
        .unwrap();
    assert_eq!(focus.caption(), None);
    for _ in 0..26 {
        focus.advance();
    }
    assert_eq!(focus.caption(), Some(1151));
    //The presentation gate clears the child when a modal takes input.
    focus.clear_child();
    assert_eq!(focus.caption(), None);
    state.prestige = None;
    let [l, t, r, b] = layout.buttons[1];
    assert_eq!(target(&state, [(l + r) / 2., (t + b) / 2.]), None);
    for (action, id) in [
        (Action::City, 1146),
        (Action::Player, 1147),
        (Action::Toggle, 1148),
        (Action::Poll, 12),
    ] {
        assert_eq!(icon_tooltip(action), Some(id));
    }
}
