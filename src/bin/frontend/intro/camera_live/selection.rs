//! By-value source inputs. No Scene, worker request or orbit approximation.
use super::owner::Owner;
use openmt2001::camera::{bank::SelectedActorInput, primary::BlockInitialization};

/// A live building identity and exactly the fields read by 481cb0. Produce
/// on Host's thread BEFORE its synchronous camera request. These words are
/// not obtained from Inspection.scene, a saved snapshot or Status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectedBuilding {
    pub business_slot: u32,
    pub building_slot: u32,
    pub category_id: u32,
    pub selected_xz_words: [u32; 2],
    pub building_kind: u32,
    pub entrance_words: [u32; 3],
}

/// Ordinary selection does not reset yaw. Only the strategic double-click
/// 4c3c24 caller adds ResetYaw before the SAME block initializer.
pub enum BlockCaller { SelectionDispatch, StrategicDoubleClick }

pub fn block(
    assets: &crate::selection::Assets, category_id: u32,
    height_word: u32, radius_word: u32,
) -> Result<BlockInitialization, String> {
    assets.camera_block_initialization(category_id, height_word, radius_word)
}

pub fn actor(
    owner: &Owner, assets: &crate::selection::Assets, selected: SelectedBuilding, auto_rotate: bool,
) -> Result<SelectedActorInput, String> {
    let capture = owner.capture();
    let parameter = u32::from_le_bytes(capture.records()[0][0xa8..0xac].try_into().unwrap());
    if parameter == 2 {
        // This branch reads no actor/model/entrance/kind. Poison unused
        // float operands deliberately; they are NOT a fallback camera pose.
        // No model lookup can reject an operation that never reads a model.
        return Ok(SelectedActorInput {
            selected_xz_words: selected.selected_xz_words,
            actor_translation: [f32::NAN; 3], actor_model_center: [f32::NAN; 3],
            building_kind: selected.building_kind,
            entrance: selected.entrance_words.map(f32::from_bits), auto_rotate,
        });
    }
    let block = assets.camera_block_initialization(selected.category_id, 0, 0)?;
    // 462f30 -> category key -> descriptor_at(key). The selected actor is the
    // CATEGORY pavement, not the selected building's registered mesh/0.5 Y.
    // Every category/pavement operand is immutable here and already loaded.
    // Dynamic entrance/XZ words above remain MOV data until the core reads
    // their actual branch (parameter2, no-auto, auto-landscape or auto-other).
    Ok(SelectedActorInput {
        selected_xz_words: selected.selected_xz_words,
        actor_translation: block.actor_translation.map(f32::from_bits),
        actor_model_center: block.model_offsets.map(f32::from_bits),
        building_kind: selected.building_kind,
        entrance: selected.entrance_words.map(f32::from_bits),
        auto_rotate,
    })
}

/// Camera suffix only, AFTER the producer has completed the applicable
/// Picked3D/selection/UI prelude and reloaded the resulting selected actor.
pub fn initialize_block(
    owner: &mut Owner, assets: &crate::selection::Assets,
    category_id: u32, caller: BlockCaller,
) -> Result<(), String> {
    let input = block(assets, category_id, 120f32.to_bits(), 80f32.to_bits())?;
    if matches!(caller, BlockCaller::StrategicDoubleClick) { owner.reset_yaw(); }
    owner.initialize_block(input)?;
    owner.set_kind(12);
    Ok(())
}

pub fn initialize_actor(
    owner: &mut Owner, assets: &crate::selection::Assets,
    selected: SelectedBuilding, auto_rotate: bool,
) -> Result<(), String> {
    let input = actor(owner, assets, selected, auto_rotate)?;
    owner.initialize_actor(input)?;
    owner.set_kind(13);
    Ok(())
}
