//! Selected-level objective input (48d740/4dacb0), separate from86ce60.
use crate::{
    level_catalog::Level,
    lua4::{Runtime, Value},
    pe::PeImage,
};

pub const GET_TARGET_CALLBACK: u32 = 742;

/// Snapshot of the selected record's binary32 field, never world objective.target.
/// Host must refresh after selection/reselection, +870 edits, and reconstruction;
/// clear on deselection/teardown. This is transient binding state, not save data.
#[derive(Default)]
pub struct Selection(Option<f32>);
impl Selection {
    pub fn bind(&mut self, level: Option<&Level>) {
        self.0 = level.map(Level::objective_target);
    }
    pub fn target(&self) -> Result<Option<f64>, String> {
        self.0.map(|v| Some(f64::from(v))).ok_or_else(|| {
            "Game.GetObjectiveTarget requires an explicitly bound selected level".into()
        })
    }
}

/// Original registration4db128..4db1ac: NONE uses PUSH imm8 zero;
/// remaining constants use binary32 immediate operands.
pub fn register_constants(vm: &mut Runtime, image: &PeImage<'_>) -> Result<(), String> {
    vm.set_number("OBJECTIVE_NONE", 0.)?;
    for (name, immediate) in [
        ("OBJECTIVE_FIRST_TO_BE_MAYOR", 0x4db13f),
        ("OBJECTIVE_WEAKEST_LINK", 0x4db153),
        ("OBJECTIVE_PROFIT", 0x4db167),
        ("OBJECTIVE_PAPERWEALTH_BY_DATE", 0x4db17b),
        ("OBJECTIVE_PAPERWEALTH_BY_AMOUNT", 0x4db18f),
        ("OBJECTIVE_SALES", 0x4db1a3),
    ] {
        vm.set_number(name, f64::from(image.f32_at(immediate)?))?;
    }
    Ok(())
}

/// Same converted SetObjective arguments used by the retained world binding.
pub(super) fn set_objective(
    state: &mut crate::scenario_objective::State,
    args: &[Value],
) -> Result<(), String> {
    let [Value::Number(kind), Value::Number(target), flag] = args else {
        return Err("invalid converted objective arguments".into());
    };
    let flag = match flag {
        Value::Number(value) => Some(*value),
        Value::Nil => None,
        _ => return Err("invalid optional objective number".into()),
    };
    state.set(*kind, *target, flag);
    Ok(())
}

/// Buffer-only selection48d740(id,1): load the supplied hub's definitions,
/// then dispatch SetObjective (data541e34), not EVENTINITIALISE_Level.
/// No initial preferences, map, city, clocks, people or hourly events are run.
/// Unsupported top-level callbacks fail rather than fabricate city state.
/// The caller resolves the selected record's directory to this exact hub.
pub fn evaluate_setup(
    image: &PeImage<'_>,
    level: &Level,
    hub: &[u8],
) -> Result<crate::scenario_objective::State, String> {
    let mut vm = Runtime::new()?;
    register_constants(&mut vm, image)?;
    vm.register("GetObjectiveTarget", GET_TARGET_CALLBACK)?;
    vm.register_coerced("SetObjective", 4, "nnN")?;
    vm.execute(
        "setup objective bindings",
        b"Game={GetObjectiveTarget=GetObjectiveTarget,SetObjective=SetObjective}",
        &mut |_, _| Err("unexpected binding callback".into()),
    )?;
    let mut selection = Selection::default();
    selection.bind(Some(level));
    let mut objective = crate::scenario_objective::State::default();
    let mut writes = 0;
    let mut handler = |id, args: &[Value]| match id {
        GET_TARGET_CALLBACK => selection.target(),
        4 => {
            set_objective(&mut objective, args)?;
            writes += 1;
            Ok(None)
        }
        _ => Err("unsupported setup objective callback".into()),
    };
    vm.execute("selected level hub", hub, &mut handler)?;
    vm.call("EVENT_SetObjective", &[0.], &mut handler)?;
    if writes == 0 {
        return Err("selected hub did not set an objective".into());
    }
    Ok(objective)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires supplied original PE and seven original hub files"]
    fn all_original_setup_objectives_and_retained_selection() {
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let root = crate::fixtures::path("SCRIPTS");
        let mut vm = Runtime::new().unwrap();
        let mut bindings = super::super::Bindings::register(&mut vm, &image).unwrap();
        let mut catalog = crate::level_catalog::Catalog::default();
        for (i, expected_kind) in [3, 6, 4, 5, 1, 2, 0].into_iter().enumerate() {
            let directory = format!("net{i}");
            // Case-independent host fixture lookup; original paths vary in case.
            let path = std::fs::read_dir(&root)
                .unwrap()
                .map(|e| e.unwrap().path())
                .find(|p| {
                    p.file_name()
                        .unwrap()
                        .to_string_lossy()
                        .eq_ignore_ascii_case(&directory)
                })
                .unwrap();
            let hub = std::fs::read(path.join("hub.lua")).unwrap();
            catalog
                .register(
                    crate::level_catalog::Registration {
                        id: 300000 + i as i32,
                        directory: directory.as_bytes().to_vec(),
                        labels: crate::level_catalog::Labels::Literal {
                            title: vec![],
                            description: vec![],
                        },
                        checksum: 0,
                    },
                    false,
                    true,
                )
                .unwrap();
            let index = catalog.find(300000 + i as u32).unwrap();
            for target in [1234.25f32 + i as f32, 9876.5 + i as f32] {
                catalog.levels[index].set_objective_target(target);
                assert!(catalog.select(300000 + i as u32, true));
                let level = catalog.selected_level().unwrap();
                let result = evaluate_setup(&image, level, &hub).unwrap();
                assert_eq!(result.kind, expected_kind, "{directory}");
                assert_eq!(result.target.to_bits(), target.to_bits(), "{directory}");
                bindings.bind_selected_level(Some(level));
                let mut world = crate::scenario_objective::State::default();
                vm.execute("selected hub", &hub, &mut |_, _| {
                    Err("unexpected hub callback".into())
                })
                .unwrap();
                let mut handler = |id, args: &[Value]| match id {
                    GET_TARGET_CALLBACK => bindings.selected_objective.target(),
                    4 => {
                        set_objective(&mut world, args)?;
                        Ok(None)
                    }
                    _ => Err("unexpected objective callback".into()),
                };
                vm.call("EVENT_SetObjective", &[0.], &mut handler).unwrap();
                assert_eq!(world.kind, expected_kind);
                assert_eq!(world.target.to_bits(), target.to_bits());
                // Retained Lua functions read selection again after active state changes.
                world.target = -91.;
                vm.execute(
                    "observe retained getter",
                    b"observed=Game.GetObjectiveTarget()",
                    &mut |id, _| {
                        assert_eq!(id, GET_TARGET_CALLBACK);
                        bindings.selected_objective.target()
                    },
                )
                .unwrap();
                assert_eq!(vm.number("observed").unwrap(), Some(f64::from(target)));
            }
        }
        assert!(catalog.select(300000, true));
        bindings.bind_selected_level(catalog.selected_level());
        assert_eq!(bindings.selected_objective.target().unwrap(), Some(9876.5));
        vm.execute(
            "reselected retained getter",
            b"observed=Game.GetObjectiveTarget()",
            &mut |id, _| {
                assert_eq!(id, GET_TARGET_CALLBACK);
                bindings.selected_objective.target()
            },
        )
        .unwrap();
        assert_eq!(vm.number("observed").unwrap(), Some(9876.5));
        bindings.bind_selected_level(None);
        assert!(bindings.selected_objective.target().is_err());
    }
}
