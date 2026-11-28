//! Audited nested sources used by the sixteen original campaign hubs.
use openmt2001::lua4::Runtime;
use std::path::Path;

pub fn register_sources(runtime: &mut Runtime, root: &Path) -> Result<(), String> {
    for map in [
        "tempmap",
        "apartmentrim",
        "apartments2",
        "parkfiller",
        "goodsparsemap",
        "goodspread",
        "sparsemap",
        "cheapbuiltmap",
        "mapbuiltupmore",
        "mapbuiltup",
        "developedmap",
        "cheapbuiltbigmap",
        "monopolymap",
        "shoppingmap",
    ] {
        let relative = format!("MAPS/{map}.lua");
        let bytes = std::fs::read(root.join(&relative)).map_err(|e| format!("{relative}: {e}"))?;
        runtime.script_buffer(&format!("scripts/{relative}"), &bytes)?;
    }
    // Scenario5 reuses3 and7 reuses6. The unused5/7 preferences are not loaded.
    for scenario in [1, 2, 3, 4, 6, 8, 9, 10, 11, 12, 13, 14, 15, 16] {
        let relative = format!("SCENARIO{scenario}/initialpref.lua");
        let bytes = std::fs::read(root.join(&relative)).map_err(|e| format!("{relative}: {e}"))?;
        runtime.script_buffer(&format!("scripts/{relative}"), &bytes)?;
    }
    // Original Tutorial 3's initializer explicitly reuses Tutorial 1's
    // supplied ownership/population preferences.
    for tutorial_preferences in [
        "TUTORIAL1/initialpref.lua",
        "TUTORIAL4/initialpref.lua",
        "TUTORIAL8/initialpref.lua",
        "TUTORIAL9/initialpref.lua",
        "TUTORIAL12/initialpref.lua",
    ] {
        let bytes = std::fs::read(root.join(tutorial_preferences))
            .map_err(|error| format!("{tutorial_preferences}: {error}"))?;
        runtime.script_buffer(&format!("scripts/{tutorial_preferences}"), &bytes)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "session_scenarios_tests.rs"]
mod tests;
