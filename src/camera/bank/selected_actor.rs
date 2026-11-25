//! Recovered selected-actor initializer00481cb0, with explicit input and trig. Liveness and UI transitions belong to its caller.
use super::{Bank, RawRecord, put, word};
use crate::{camera::primary::AngularTrig, x87::Sum};
use std::cmp::Ordering;

/// Captured actor/building data at the original selected-actor setup call.
/// Model center must be resolved by the caller, never defaulted on lookup error.
pub struct SelectedActorInput {
    pub selected_xz_words: [u32; 2],
    pub actor_translation: [f32; 3],
    pub actor_model_center: [f32; 3],
    pub building_kind: u32,
    pub entrance: [f32; 3],
    pub auto_rotate: bool,
}
type Result<T> = std::result::Result<T, &'static str>;
fn ext(raw: &RawRecord, at: usize) -> Result<Sum> {
    Sum::from_f32(f32::from_bits(word(raw, at)))
}
fn store(raw: &mut RawRecord, at: usize, value: Sum) -> Result<()> {
    let v = value.to_f32();
    if !v.is_finite() {
        return Err("selected-actor store overflow");
    }
    put(raw, at, v.to_bits());
    Ok(())
}
fn difference(a: f32, b: f32) -> Result<Sum> {
    let mut v = Sum::from_f32(a)?;
    v.add(Sum::from_f32(b)?.negated());
    Ok(v)
}

impl Bank {
    /// Callback initializer only; the caller's subsequent mode13 kind setter
    /// and separate UI/selection operations do not run here. All writes stage
    /// atomically; numeric provider side effects cannot be rolled back.
    pub fn initialize_selected_actor(
        &mut self,
        input: SelectedActorInput,
        trig: &mut impl AngularTrig,
    ) -> Result<()> {
        self.try_update_primary(|raw| {
            raw[0x70..0x73].fill(1);
            put(raw, 0x74, 20);
            if word(raw, 0xa8) == 2 {
                put(raw, 0xb4, input.selected_xz_words[0]);
                put(raw, 0xb8, 0);
                put(raw, 0xbc, input.selected_xz_words[1]);
            } else if !input.auto_rotate {
                put(raw, 0x98, word(raw, 0x7c));
                for j in 0..3 {
                    let mut v = Sum::from_f32(input.actor_translation[j])?;
                    v.add(Sum::from_f32(input.actor_model_center[j])?);
                    store(raw, 0xb4 + j * 4, v)?;
                }
                put(raw, 0x38, 90f32.to_bits());
                put(raw, 0xb0, 75f32.to_bits());
            } else {
                let (across, forward) = if input.building_kind == 2 {
                    (
                        difference(
                            input.actor_translation[0],
                            f32::from_bits(input.selected_xz_words[0]),
                        )?,
                        difference(
                            input.actor_translation[2],
                            f32::from_bits(input.selected_xz_words[1]),
                        )?
                        .to_f32(),
                    )
                } else {
                    (
                        Sum::from_f32(input.entrance[0])?.negated(),
                        -input.entrance[2],
                    )
                };
                if !forward.is_finite() {
                    return Err("selected-actor forward spill overflow");
                }
                let mut x = Sum::from_f32(input.actor_translation[0])?;
                x.add(across.negated());
                store(raw, 0xb4, x)?;
                let z = difference(input.actor_translation[2], forward)?;
                put(raw, 0xb8, 20f32.to_bits());
                put(raw, 0x38, 70f32.to_bits());
                put(raw, 0xb0, 45f32.to_bits());
                store(raw, 0xbc, z)?;
                let abs_forward = Sum::from_f32(forward)?.ftol_low_word().wrapping_abs();
                let abs_across = across.ftol_low_word().wrapping_abs();
                let a = across.compare(Sum::default());
                let f = Sum::from_f32(forward)?.compare(Sum::default());
                let goal = match abs_forward.cmp(&abs_across) {
                    Ordering::Greater => match f {
                        Ordering::Less => Some(0x3fc90fdb),
                        Ordering::Greater => Some(0xbfc90fdb),
                        _ => None,
                    },
                    Ordering::Equal => match (a, f) {
                        (Ordering::Less, Ordering::Greater) => Some(0xbf490fdb),
                        (Ordering::Less, Ordering::Less) => Some(0x3f490fdb),
                        (Ordering::Greater, Ordering::Greater) => Some(0xc016cbe4),
                        (Ordering::Greater, Ordering::Less) => Some(0x4016cbe4),
                        _ => None,
                    },
                    Ordering::Less => match a {
                        Ordering::Less => Some(0),
                        Ordering::Greater => Some(0x40490fdb),
                        _ => None,
                    },
                };
                if let Some(goal) = goal {
                    put(raw, 0x98, goal);
                }
                let tau = Sum::from_f32(f32::from_bits(0x40c90fdb))?;
                let mut yaw = ext(raw, 0x7c)?;
                if yaw.compare(tau) == Ordering::Greater {
                    yaw.add(tau.negated());
                    store(raw, 0x7c, yaw)?;
                }
                yaw = ext(raw, 0x7c)?;
                if yaw.compare(tau.negated()) == Ordering::Less {
                    yaw.add(tau);
                    store(raw, 0x7c, yaw)?;
                }
                let mut delta = ext(raw, 0x7c)?;
                delta.add(ext(raw, 0x98)?.negated());
                let pi = Sum::from_f32(f32::from_bits(0x40490fdb))?;
                if delta.compare(pi) == Ordering::Greater {
                    let mut goal = ext(raw, 0x98)?;
                    goal.add(tau);
                    store(raw, 0x98, goal)?;
                } else if delta.compare(pi.negated()) == Ordering::Less {
                    let mut goal = ext(raw, 0x98)?;
                    goal.add(tau.negated());
                    store(raw, 0x98, goal)?;
                }
            }
            let sine = trig.sine(ext(raw, 0x7c)?)?;
            store(raw, 0x88, sine)?;
            let cosine = trig.cosine(ext(raw, 0x7c)?)?;
            store(raw, 0x90, cosine)?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests;
