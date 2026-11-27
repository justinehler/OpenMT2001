//! Retained non-road transport state shared by hourly and frame dispatch.
use super::{HourlyEffects, HourlyInputs, RouteGeometry, RouteSound, models::Models};

pub struct Scene {
    pub hourly: HourlyEffects,
    pub aircraft: [[u8; 304]; 8],
    pub trains: [[u8; 60]; 10],
    pub environment: Environment,
    pub first_frame: bool,
}
#[derive(Default)]
pub struct Environment {
    pub crane_position: [f32; 3],
    pub ship_position: [f32; 3],
    pub frame: u32,
    pub sequence_tick: u32,
    pub sequence_active: u8,
    pub frozen: u8,
}
#[derive(Clone, Copy, Debug)]
pub enum EnvironmentCall {
    Ship(u32),
    Cue(u32),
    Refresh,
}
#[derive(Clone, Copy, Debug)]
pub enum LogicEvent {
    Environment {
        call: EnvironmentCall,
        position: [f32; 3],
    },
    Sound(RouteSound),
}
impl Scene {
    /// Original0044f0b0 frame order. Submissions carry original auxiliary
    /// bytes; the host can render them without owning their lifecycle.
    pub fn advance_frame(
        &mut self,
        tick: u32,
        models: &Models<'_>,
        prior: [u8; 284],
        mut submit: impl FnMut(&mut [u8; 284]) -> Result<(), String>,
    ) -> Result<(), String> {
        if self.first_frame {
            for index in 0..4 {
                let slot = self.hourly.effects[..8]
                    .iter()
                    .position(|row| super::word(row, 0) == 0)
                    .unwrap_or(8);
                let row = &mut self.hourly.effects[slot];
                super::put(row, 0, 2);
                super::put(row, 4, 0x688db0 + index * 16);
                super::put(row, 8, 0x687790 + index * 16);
                super::put(row, 12, 0);
                super::put(row, 16, tick.wrapping_sub(1));
            }
            self.first_frame = false;
        }
        for index in 0..8 {
            if super::word(&self.aircraft[index], 0) != 0 {
                self.aircraft_frame(index, tick, models, &mut submit)?;
            }
        }
        for row in &self.trains {
            if super::word(row, 0) == 0 {
                continue;
            }
            let mut auxiliary = prior;
            crate::resident::initialize_auxiliary(&mut auxiliary);
            models.bind(
                0x688880_u32.wrapping_add(super::word(row, 4).wrapping_mul(16)),
                &mut auxiliary,
            )?;
            let mut matrix = models.locator_matrix(
                0x6860e8_u32.wrapping_add(super::word(row, 8).wrapping_mul(16)),
                1,
            )?;
            matrix[9..].copy_from_slice(&[
                f32::from_bits(super::word(row, 0x18)),
                f32::from_bits(super::word(row, 0x1c)),
                -f32::from_bits(super::word(row, 0x20)),
            ]);
            for (i, value) in matrix.iter().enumerate() {
                super::put(&mut auxiliary, 0xbc + i * 4, value.to_bits());
            }
            auxiliary[0xb8] &= !8;
            submit(&mut auxiliary)?;
        }
        for row in &self.hourly.effects[..8] {
            if super::word(row, 0) == 0 {
                continue;
            }
            let mut auxiliary = prior;
            crate::resident::initialize_auxiliary(&mut auxiliary);
            models.bind(super::word(row, 4), &mut auxiliary)?;
            let matrix = models.locator_matrix(super::word(row, 8), self.environment.frame)?;
            for (i, value) in matrix.iter().enumerate() {
                super::put(&mut auxiliary, 0xbc + i * 4, value.to_bits());
            }
            auxiliary[0xb8] &= !8;
            submit(&mut auxiliary)?;
        }
        // The final44e0b0 loop is an original bare RET.
        Ok(())
    }
    pub fn advance_logic(
        &mut self,
        image: &crate::pe::PeImage<'_>,
        models: &Models<'_>,
        tick: u32,
        camera: [f32; 3],
        mut host: impl FnMut(LogicEvent) -> Result<(), String>,
    ) -> Result<(), String> {
        self.environment_tick(tick, models, |call, scene| {
            let position = match call {
                EnvironmentCall::Ship(_) => scene.environment.ship_position,
                EnvironmentCall::Cue(_) | EnvironmentCall::Refresh => {
                    scene.environment.crane_position
                }
            };
            host(LogicEvent::Environment { call, position })
        })?;
        let constants = super::train_motion::Constants::from_original(image, camera)?;
        for (index, row) in self.trains.iter_mut().enumerate() {
            if super::word(row, 0) != 0 {
                super::train_motion::advance(
                    index,
                    row,
                    constants,
                    &mut super::train_motion::Callbacks {
                        audio: &mut |call, _| {
                            host(LogicEvent::Sound(RouteSound {
                                id: call.kind,
                                distance: f32::from_bits(call.arguments[1]),
                                arguments: [0; 2],
                            }))
                        },
                    },
                )?;
            }
        }
        self.proximity_sounds(&mut { tick }, &mut { camera }, |call, _, _, _| {
            host(LogicEvent::Sound(call))
        })
    }
    /// Original0044ec50 environment prefix, preceding train motion/audio.
    pub fn environment_tick(
        &mut self,
        tick: u32,
        models: &Models<'_>,
        mut callback: impl FnMut(EnvironmentCall, &mut Self) -> Result<(), String>,
    ) -> Result<(), String> {
        self.environment.crane_position = models.crane_position()?;
        if super::word(&self.hourly.effects[0], 12) == 1 {
            if self.environment.frozen == 0 && super::word(&self.hourly.effects[0], 0) != 0 {
                self.environment.frame = self.environment.frame.wrapping_add(1);
            }
            if super::word(&self.hourly.effects[0], 0) != 0 {
                let mesh = models.mesh(super::word(&self.hourly.effects[0], 8))?;
                if self.environment.frame as i32 >= mesh.frames as i32 {
                    super::put(&mut self.hourly.effects[0], 0, 0);
                    super::put(&mut self.hourly.effects[0], 16, tick.wrapping_sub(1));
                    self.environment.frame = 1;
                }
                let point = mesh
                    .positions(self.environment.frame as usize)?
                    .first()
                    .copied()
                    .ok_or("ship locator has no vertices")?;
                self.environment.ship_position = [point[0], point[1], -point[2]];
                callback(EnvironmentCall::Ship(self.hourly.model_choice), self)?;
            }
        }
        if self.environment.sequence_active == 1 {
            self.environment.sequence_tick = self.environment.sequence_tick.wrapping_add(1);
            // Native comparisons reload this global after each audio call.
            for (at, cues) in [
                (2, &[29][..]),
                (300, &[31, 30][..]),
                (800, &[29, 32][..]),
                (1400, &[30, 31][..]),
            ] {
                if self.environment.sequence_tick == at {
                    for &cue in cues {
                        callback(EnvironmentCall::Cue(cue), self)?;
                    }
                }
            }
            callback(EnvironmentCall::Refresh, self)?;
        }
        if self.environment.sequence_tick as i32 >= 1790 && self.environment.frozen == 1 {
            self.environment.sequence_active = 0;
            self.environment.frozen = 0;
            self.environment.sequence_tick = 1;
            callback(EnvironmentCall::Cue(32), self)?;
            callback(EnvironmentCall::Cue(28), self)?;
            callback(EnvironmentCall::Refresh, self)?;
        } else if self.hourly.model_choice == 1
            && self.environment.frame == 1500
            && self.environment.sequence_active == 0
        {
            self.environment.sequence_active = 1;
            self.environment.frozen = 1;
            callback(EnvironmentCall::Cue(27), self)?;
            callback(EnvironmentCall::Refresh, self)?;
        }
        Ok(())
    }
    /// Original0044e000 frame consumer. The host receives the actual auxiliary
    /// for4e0d00 submission; lifetime/pose changes remain in the shared state.
    pub fn aircraft_frame(
        &mut self,
        index: usize,
        tick: u32,
        models: &Models<'_>,
        mut submit: impl FnMut(&mut [u8; 284]) -> Result<(), String>,
    ) -> Result<(), String> {
        let row = self
            .aircraft
            .get_mut(index)
            .ok_or("aircraft index outside pool")?;
        let elapsed = tick.wrapping_sub(super::word(row, 16));
        let reference = 0x685cc8_u32.wrapping_add(super::word(row, 8).wrapping_mul(16));
        let matrix = models.locator_matrix(reference, elapsed)?;
        let frames = models.mesh(reference)?.frames;
        let auxiliary: &mut [u8; 284] = (&mut row[20..]).try_into().unwrap();
        for (i, value) in matrix.into_iter().enumerate() {
            super::put(auxiliary, 0xbc + i * 4, value.to_bits());
        }
        for axis in 0..3 {
            super::put(auxiliary, 4 + axis * 4, matrix[9 + axis].to_bits());
        }
        if (elapsed as i32) >= frames as i32 {
            super::put(row, 0, 0);
        } else {
            auxiliary[0xb8] &= !8;
            submit(auxiliary)?;
        }
        Ok(())
    }
    /// First-process zero-filled transport pools. Reloads must retain/restore
    /// their state instead. Static boats are admitted separately by44f0b0 on
    /// the first rendered frame, so this does not pretend that frame occurred.
    pub fn fresh(image: &crate::pe::PeImage<'_>, random: [u32; 3]) -> Result<Self, String> {
        let mut models = [0; 4];
        for (i, model) in models.iter_mut().enumerate() {
            *model = u32::from_le_bytes(image.at(0x539d50 + i as u32 * 4, 4)?.try_into().unwrap());
        }
        let mut state = Self {
            hourly: HourlyEffects {
                tier: 0,
                random,
                model_choice: 0,
                variant: 0,
                auxiliaries: [[0; 300]; 2],
                effects: [[0; 20]; 9],
                models,
            },
            aircraft: [[0; 304]; 8],
            trains: [[0; 60]; 10],
            environment: Environment {
                crane_position: [0.; 3],
                ship_position: [0.; 3],
                frame: 0,
                sequence_tick: u32::from_le_bytes(image.at(0x539b7c, 4)?.try_into().unwrap()),
                sequence_active: 0,
                frozen: 0,
            },
            first_frame: true,
        };
        // CRT0044df30 constructs every embedded auxiliary through4df9b0,
        // which delegates to the shared4df9c0 initializer before gameplay.
        for row in &mut state.aircraft {
            crate::resident::initialize_auxiliary((&mut row[20..]).try_into().unwrap());
        }
        for row in &mut state.hourly.auxiliaries {
            crate::resident::initialize_auxiliary((&mut row[16..]).try_into().unwrap());
        }
        Ok(state)
    }
    /// Compose the original hourly scheduler with actual retained pools,
    /// locator geometry and model definitions. Audio submission remains host I/O.
    pub fn advance_hour(
        &mut self,
        inputs: &mut HourlyInputs,
        models: &Models<'_>,
        routes: &[RouteGeometry; 8],
        camera: [f32; 3],
        mut sound: impl FnMut(RouteSound, &mut [[u8; 60]; 10]) -> Result<(), String>,
    ) -> Result<(), String> {
        self.hourly.advance_with_secondary(
            &mut self.aircraft,
            inputs,
            |reference, auxiliary, _| models.bind(reference, auxiliary),
            |tier, route, _, inputs| {
                super::spawn_route(
                    &mut self.trains,
                    tier,
                    route,
                    inputs.tick,
                    |index| {
                        routes
                            .get(index as usize)
                            .copied()
                            .ok_or_else(|| "transport route outside table".into())
                    },
                    camera,
                    &mut sound,
                )?;
                Ok(())
            },
        )
    }
}
