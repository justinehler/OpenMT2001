//! Original4655f0/465c40 auction scene records and4b4580 presentation.
use openmt2001::{
    archive::ArchiveIndex, mesh::CachedMesh, pe::PeImage, render::SurfaceMode,
    render_scene::OwnedDraw, scene_assets,
};

const PATHS: [u32; 14] = [
    0x546724, 0x5466f0, 0x5466c8, 0x5466ac, 0x546c8c, 0x546c60, 0x546c38, 0x546c10, 0x546be8,
    0x546bc0, 0x546b9c, 0x546b74, 0x546b48, 0x546b18,
];
const RESULT_PATHS: [u32; 21] = [
    0x546aec, 0x546abc, 0x546a90, 0x546a64, 0x546a38, 0x546a08, 0x5469d8,
    0x5469a8, 0x546978, 0x546948, 0x54691c, 0x5468ec, 0x5468c0, 0x546894,
    0x546868, 0x546838, 0x54680c, 0x5467e0, 0x5467b4, 0x546784, 0x5474fc,
];

#[derive(Clone, Debug)]
pub struct Pose {
    pub model: usize,
    pub position: [f32; 3],
    pub yaw: f32,
    pub frame: usize,
    pub flags: u16,
}

/// Original scene rows retain their insertion order. The dynamic row0 chair
/// is intentional: retail465c30 (auctioneer animation) is a literal RET.
pub fn initial_layout(characters: &[u32]) -> Result<Vec<Pose>, String> {
    if characters.is_empty() || characters.len() > 6 || characters.iter().any(|&c| c >= 10) {
        return Err("Auction scene requires one to six original characters".into());
    }
    let row = |model, position, flags| Pose {
        model,
        position,
        yaw: 0.,
        frame: 0,
        flags,
    };
    let mut result = vec![row(0, [0.; 3], 9), row(1, [0., 2., 37.], 9)];
    for position in [
        [14., 0., -2.],
        [14., 0., 22.],
        [-14., 0., -2.],
        [-14., 0., 22.],
    ] {
        result.push(row(2, position, 9));
    }
    result.push(row(3, [0., 2., 37.], 0));
    let mut x = ((characters.len() + 1) as f64 * f64::from(1.4_f32)) as f32;
    for &character in characters {
        x = (f64::from(x) - f64::from(2.8_f32)) as f32;
        result.push(Pose {
            model: 4 + character as usize,
            position: [x, 0.2, 20.],
            yaw: 180.,
            frame: 1,
            flags: 0,
        });
    }
    let mut x = (-((characters.len() + 1) as f64) * f64::from(1.4_f32)) as f32;
    for _ in characters {
        x = (f64::from(x) + f64::from(2.8_f32)) as f32;
        result.push(Pose {
            model: 3,
            position: [x, 0.2, 20.],
            yaw: 180.,
            frame: 0,
            flags: 9,
        });
    }
    Ok(result)
}

///4b3130 submits the winner standing and loser sitting at fixed city positions.
/// Character10 is the original Mr Monopoly fallback in the standing slot;
/// the sitting slot is omitted for that sentinel.
pub fn result_layout(characters: (u32, u32)) -> Result<Vec<Pose>, String> {
    if characters.0 > 10 || characters.1 > 10 {
        return Err("Result actor character outside original roster".into());
    }
    let mut poses = vec![Pose {
        model: if characters.0 == 10 { 20 } else { characters.0 as usize },
        position: [f32::from_bits(0xc256f7cf), f32::from_bits(0x407fbe77), f32::from_bits(0x423f9db2)],
        yaw: 0., frame: 0, flags: 8,
    }];
    if characters.1 != 10 {
        poses.push(Pose {
            model: 10 + characters.1 as usize,
            position: [f32::from_bits(0xc256020c), 4., f32::from_bits(0x42338a3d)],
            yaw: 0., frame: 0, flags: 8,
        });
    }
    Ok(poses)
}

struct Model {
    frames: Vec<Vec<[f32; 3]>>,
    parts: Vec<(openmt2001::people::PreparedPart, usize)>,
    uvs: Vec<[f32; 2]>,
}
pub struct Assets {
    models: Vec<Model>,
    surfaces: Vec<OwnedDraw>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActorCue {
    Bid { participant: u32, character: u32, suppressed: bool },
    Withdraw { participant: u32, character: u32, suppressed: bool },
    Win { character: u32 },
    Applause,
}

#[derive(Clone, Copy, Debug)]
struct FrameRange {
    start: u16,
    end: u16,
}

#[derive(Clone, Debug)]
struct Actor {
    character: u32,
    ranges: [FrameRange; 3],
    frame: u16,
    end: u16,
    pending: u32,
}

/// The original 465c40 calls 465980(kind 1), then kind 7 for each actor.
/// Kind 7 installs a type-2 4eccc0 animation whose 4ecc20 completion callback
/// dispatches the current pending kind. 4ece30 advances it once per logic tick.
pub struct Actors {
    pub poses: Vec<Pose>,
    rows: Vec<Actor>,
    bid_voice_suppressed: bool,
}

impl Actors {
    pub fn from_original(image: &PeImage<'_>, assets: &Assets, characters: &[u32]) -> Result<Self, String> {
        let poses = initial_layout(characters)?;
        let mut rows = Vec::with_capacity(characters.len());
        for &character in characters {
            let read = |address: u32| -> Result<u16, String> {
                let at = address + character * 4;
                Ok(u16::from_le_bytes(image.at(at, 2)?.try_into().unwrap()))
            };
            // 4659f4/465a37/465b7f pass these exact start/end table words
            // to 4ecde0. The end is exclusive in the 4ece30 comparison.
            let ranges = [
                FrameRange { start: read(0x53c55c)?, end: read(0x53c584)? },
                FrameRange { start: read(0x53c5ac)?, end: read(0x53c5d4)? },
                FrameRange { start: read(0x53c5fc)?, end: read(0x53c624)? },
            ];
            let model = assets.models.get(4 + character as usize).ok_or("Auction actor model")?;
            for range in ranges {
                if range.start >= range.end || usize::from(range.end) > model.frames.len() {
                    return Err(format!("Auction character {character} has invalid original frame range"));
                }
            }
            rows.push(Actor {
                character,
                ranges,
                frame: ranges[0].start,
                end: ranges[0].end,
                pending: 1,
            });
        }
        Ok(Self { poses, rows, bid_voice_suppressed: false })
    }

    pub fn set_bid_voice_suppressed(&mut self, suppressed: bool) {
        self.bid_voice_suppressed = suppressed;
    }

    /// 464769 plays the third character voice immediately after the auction
    /// rotation reports no winner; it does not wait for an actor animation.
    pub fn withdraw(&self, participant: u32) -> Result<ActorCue, String> {
        let row = self.rows.get(participant as usize)
            .ok_or("Auction withdraw participant outside prepared scene")?;
        Ok(ActorCue::Withdraw { participant, character: row.character,
            suppressed: self.bid_voice_suppressed })
    }

    /// Incoming kind 2/5 only changes the stored state at 7f92bc. It does not
    /// play a sound or replace the running animation before its callback.
    pub fn effect(&mut self, participant: u32, kind: u32) -> Result<(), String> {
        if !(1..=5).contains(&kind) {
            return Err(format!("Unsupported auction actor effect {kind}"));
        }
        let row = self.rows.get_mut(participant as usize)
            .ok_or("Auction actor participant outside prepared scene")?;
        row.pending = kind;
        Ok(())
    }

    pub fn advance(&mut self) -> Vec<ActorCue> {
        let mut cues = Vec::new();
        for (participant, row) in self.rows.iter_mut().enumerate() {
            row.frame += 1;
            if row.frame >= row.end {
                let (range, next) = match row.pending {
                    1 => (row.ranges[0], 1),
                    2 => {
                        cues.push(ActorCue::Bid { participant: participant as u32,
                            character: row.character, suppressed: self.bid_voice_suppressed });
                        (row.ranges[1], 1)
                    }
                    3 => (row.ranges[0], 4),
                    4 => (row.ranges[0], 5),
                    5 => {
                        cues.push(ActorCue::Win { character: row.character });
                        cues.push(ActorCue::Applause);
                        (row.ranges[2], 1)
                    }
                    _ => unreachable!("effect validates pending kinds"),
                };
                row.frame = range.start;
                row.end = range.end;
                row.pending = next;
            }
            self.poses[7 + participant].frame = usize::from(row.frame);
        }
        cues
    }
}
impl Assets {
    pub fn prepare(
        &self,
        renderer: &openmt2001::render::Renderer,
        characters: &[u32],
    ) -> Result<openmt2001::render::PreparedScene, String> {
        let draws = self.draws(&initial_layout(characters)?)?;
        renderer.prepare(
            &draws.iter().map(OwnedDraw::borrowed).collect::<Vec<_>>(),
            Self::camera_matrix()?,
            None,
        )
    }

    pub fn refresh(
        &self,
        renderer: &openmt2001::render::Renderer,
        poses: &[Pose],
        scene: &mut openmt2001::render::PreparedScene,
    ) -> Result<(), String> {
        let draws = self.draws(poses)?;
        renderer.refresh_prepared(
            scene,
            &draws.iter().map(OwnedDraw::borrowed).collect::<Vec<_>>(),
            Self::camera_matrix()?,
            None,
        )
    }

    fn camera_matrix() -> Result<[f32; 16], String> {
        let sample = initial_camera();
        let [near, far] = openmt2001::camera::city_clip(sample.eye[1])?;
        let projection = openmt2001::camera::projection(
            sample.vertical_fov,
            near,
            far,
            1280,
            960,
            [0., 1., 0., 1.],
        )?;
        let view = openmt2001::camera::view_with_up(sample.eye, sample.target, sample.up);
        let matrix = (glam::Mat4::from_cols_array(&projection)
            * glam::Mat4::from_cols_array(&view))
        .to_cols_array();
        Ok(matrix)
    }
    pub fn load(image: &PeImage<'_>, index: &ArchiveIndex, archive: &[u8]) -> Result<Self, String> {
        Self::load_paths(image, index, archive, &PATHS, false)
    }
    pub fn load_result(image: &PeImage<'_>, index: &ArchiveIndex, archive: &[u8]) -> Result<Self, String> {
        Self::load_paths(image, index, archive, &RESULT_PATHS, true)
    }
    fn load_paths(image: &PeImage<'_>, index: &ArchiveIndex, archive: &[u8], paths: &[u32], result: bool) -> Result<Self, String> {
        let textures = if result { scene_assets::result_textures(image, index, archive, 1280)? }
            else { scene_assets::auction_textures(image, index, archive, 1280)? };
        let (pages, regions) = scene_assets::scenery_pages(&textures)?;
        let surfaces = pages
            .into_iter()
            .map(|page| {
                OwnedDraw::texture_page(
                    if openmt2001::texture::uploaded_page_flags(page.flags) & 0x8000 != 0 {
                        SurfaceMode::StaticOpaque
                    } else {
                        SurfaceMode::StaticLit
                    },
                    page.side,
                    page.side,
                    page.rgba.into(),
                )
            })
            .collect();
        let mut models = Vec::new();
        for &address in paths {
            let name = image.at(address, 260)?;
            let end = name
                .iter()
                .position(|b| *b == 0)
                .ok_or("Auction path terminator")?;
            let name = std::str::from_utf8(&name[..end]).map_err(|e| e.to_string())?;
            let entry = index
                .find(name)
                .ok_or_else(|| format!("Missing auction model {name}"))?;
            let mesh = CachedMesh::parse(
                archive
                    .get(entry.offset as usize..(entry.offset + entry.length) as usize)
                    .ok_or("Auction model range")?,
            )?;
            let materials = mesh
                .texture_names
                .iter()
                .map(|name| {
                    let key = name.rsplit('\\').next().unwrap();
                    textures
                        .iter()
                        .position(|texture| {
                            texture.name.as_ref().is_some_and(|name| {
                                name.as_bytes().get(..key.len()).is_some_and(|prefix| {
                                    prefix.eq_ignore_ascii_case(key.as_bytes())
                                })
                            })
                        })
                        .map(|i| &regions[i])
                        .or_else(|| {
                            // Original4ffb56 returns texture slot zero when
                            // these non-file material names miss registration.
                            (key.eq_ignore_ascii_case("standing_poses_phong3")
                                || key.eq_ignore_ascii_case("maya_models_white4by4.tt"))
                                .then(|| regions.first()).flatten()
                        })
                        .ok_or_else(|| format!("Unresolved auction texture {key}"))
                })
                .collect::<Result<Vec<_>, String>>()?;
            let uvs = scene_assets::scenery_mesh_uvs(
                &mesh,
                &materials.iter().map(|r| r.uv).collect::<Vec<_>>(),
            )?;
            let prepared = openmt2001::people::prepare_mesh(&mesh, &mesh)?;
            let parts = prepared
                .parts
                .into_iter()
                .map(|part| {
                    let texture =
                        u16::from_le_bytes(part.material_header[4..6].try_into().unwrap()) as usize;
                    let page = materials.get(texture).ok_or("Auction part material")?.page;
                    Ok((part, page))
                })
                .collect::<Result<_, String>>()?;
            let frames = (0..mesh.frames)
                .map(|frame| mesh.positions(frame))
                .collect::<Result<_, _>>()?;
            models.push(Model { frames, parts, uvs });
        }
        Ok(Self { models, surfaces })
    }

    pub fn draws(&self, poses: &[Pose]) -> Result<Vec<OwnedDraw>, String> {
        self.draws_lit(poses, None)
    }

    /// 004b3a2b enables the result actor's camera highlight before 004b3130.
    /// Rebuild on each camera sample: the original 004f2390 does so per draw.
    pub fn draws_result(
        &self,
        poses: &[Pose],
        light: &openmt2001::lighting::SceneLight,
        eye: [f32; 3],
    ) -> Result<Vec<OwnedDraw>, String> {
        self.draws_lit(poses, Some((light, eye)))
    }

    fn draws_lit(
        &self,
        poses: &[Pose],
        result: Option<(&openmt2001::lighting::SceneLight, [f32; 3])>,
    ) -> Result<Vec<OwnedDraw>, String> {
        let palette = if let Some((light, _)) = result {
            openmt2001::lighting::Palette::new(
                light.diffuse,
                light.ambient,
                light.diffuse,
                0,
                light.scale,
            )
        } else {
            openmt2001::lighting::Palette::new(u32::MAX, 0x3f3f3f, u32::MAX, 0, 1.)
        };
        let sun = result.map_or(
            [f32::from_bits(0x3ee8a86d), f32::from_bits(0x3f641180), 0.],
            |(light, _)| light.direction,
        );
        let rgb = |color: u32| [16, 8, 0].map(|shift| ((color >> shift) & 255) as f32 / 255.);
        let mut draws = self.surfaces.clone();
        for pose in poses {
            let model = self.models.get(pose.model).ok_or("Auction pose model")?;
            let positions = model.frames.get(pose.frame).ok_or("Auction pose frame")?;
            let radians = f64::from((f64::from(pose.yaw) * f64::from(0.017453292_f32)) as f32);
            let (s, c) = radians.sin_cos();
            let s = f64::from(s as f32);
            let matrix = [
                [c as f32, 0., s as f32],
                [0., 1., 0.],
                [(-s) as f32, 0., c as f32],
                pose.position,
            ];
            let sun = if result.is_some() {
                [sun[0], sun[1], -sun[2]]
            } else {
                sun
            };
            let local_sun = std::array::from_fn(|i| {
                (f64::from(sun[2]) * f64::from(matrix[i][2])
                    + (f64::from(sun[1]) * f64::from(matrix[i][1])
                        + f64::from(sun[0]) * f64::from(matrix[i][0]))) as f32
            });
            let half = result.map(|(_, eye)| {
                let delta = [
                    eye[0] - pose.position[0],
                    eye[1] - pose.position[1],
                    -(eye[2] - pose.position[2]),
                ];
                let local_eye = std::array::from_fn(|i| {
                    (f64::from(delta[2]) * f64::from(matrix[i][2])
                        + (f64::from(delta[1]) * f64::from(matrix[i][1])
                            + f64::from(delta[0]) * f64::from(matrix[i][0])))
                        as f32
                });
                crate::fresh_city::result::lighting::camera_half(local_sun, local_eye, [0.; 3])
            });
            let mut transform = openmt2001::people::PeopleTransform {
                matrix,
                translation_only: pose.yaw == 0.,
            };
            transform.matrix[2] = transform.matrix[2].map(|v| -v);
            for (part, page) in &model.parts {
                let draw = &mut draws[*page];
                for triangle in &part.triangles {
                    for &local in triangle {
                        let local = local as usize;
                        let source = part.source_vertices[local];
                        let p = transform.position(positions[source]);
                        let uv = model.uvs[source];
                        let shades = if let Some(half) = half {
                            crate::fresh_city::result::lighting::shade_vertex(
                                &palette,
                                part.normals[local],
                                local_sun,
                                half,
                            )?
                        } else {
                            palette.shade_static(part.normals[local], local_sun)?
                        };
                        let color = rgb(shades[0]);
                        draw.vertices
                            .push([p[0], p[1], p[2], color[0], color[1], color[2], uv[0], uv[1]]);
                        draw.specular.as_mut().unwrap().push(rgb(shades[1]));
                    }
                }
            }
        }
        draws.retain(|draw| !draw.vertices.is_empty());
        // Original4f22d0/4f2160 submit opaque queued pages before blended
        // pages. Transparent texels still write depth in the original state.
        draws.sort_by_key(|draw| u8::from(draw.mode != SurfaceMode::StaticOpaque));
        Ok(draws)
    }
}

pub fn initial_camera() -> openmt2001::flyby::CameraSample {
    let angle = f64::from(std::f32::consts::FRAC_PI_2);
    openmt2001::flyby::CameraSample {
        eye: [
            (angle.cos() * 10.) as f32,
            10.,
            (angle.sin() * 10. + 30.) as f32,
        ],
        target: [0., 5., 30.],
        up: [0., 1., 0.],
        vertical_fov: 35.,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original auction models and v1.4 animation tables"]
    fn original_auction_win_waits_for_actor_completion() {
        use openmt2001::ai_messages::offers::auction::{self, Command};
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let exe = read("OPENMT2001_EXE");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let image = PeImage::parse(&exe).unwrap();
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let assets = Assets::load(&image, &index, &archive).unwrap();
        let mut actors = Actors::from_original(&image, &assets, &[0]).unwrap();
        assert_eq!((actors.rows[0].ranges[0].start, actors.rows[0].ranges[0].end), (1, 20));
        assert_eq!((actors.rows[0].ranges[2].start, actors.rows[0].ranges[2].end), (71, 170));
        for _ in 0..5 { assert!(actors.advance().is_empty()); }
        let mut state = [0u8; 128];
        state[0x60..0x64].copy_from_slice(&0u32.to_le_bytes());
        let (sender, receiver) = std::sync::mpsc::channel();
        let mut host_auction = Some(crate::live_host::Auction::default());
        let mut omitted = std::collections::BTreeSet::new();
        auction::apply(Command { action: 4, participant: 3, amount: 0 },
            &mut state, 1, true, |call, _| {
                crate::live_host::auction_action(call, &mut host_auction, &mut omitted, 0,
                    &Some(sender.clone()), &std::cell::Cell::new(false)).map(|_| ())
            }).unwrap();
        assert!(!omitted.contains(&0x465980));
        let effects = receiver.try_iter().collect::<Vec<_>>();
        assert_eq!(effects, vec![crate::auction_bridge::ActorEvent::Effect { participant: 0, kind: 5 }]);
        for effect in effects {
            if let crate::auction_bridge::ActorEvent::Effect { participant, kind } = effect {
                actors.effect(participant, kind).unwrap();
            }
        }
        assert_eq!(actors.poses[7].frame, 6, "kind5 cannot jump to the win pose");
        for _ in 0..13 { assert!(actors.advance().is_empty()); }
        assert_eq!(actors.poses[7].frame, 19);
        assert_eq!(actors.advance(), vec![ActorCue::Win { character: 0 }, ActorCue::Applause]);
        assert_eq!(actors.poses[7].frame, 71);
        for _ in 0..98 { assert!(actors.advance().is_empty()); }
        assert_eq!(actors.poses[7].frame, 169);
        assert!(actors.advance().is_empty());
        assert_eq!(actors.poses[7].frame, 1);
        actors.effect(0, 2).unwrap();
        actors.set_bid_voice_suppressed(true);
        for _ in 0..18 { assert!(actors.advance().is_empty()); }
        assert_eq!(actors.advance(), vec![ActorCue::Bid {
            participant: 0, character: 0, suppressed: true,
        }]);
        assert_eq!(actors.poses[7].frame, 21);
        let gate = std::cell::Cell::new(true);
        crate::live_host::auction_action(auction::Call::ShowPanel(0x565b9c),
            &mut host_auction, &mut omitted, 0, &Some(sender), &gate).unwrap();
        assert!(!gate.get(), "original 4140b0 reopen must reset the bid-voice/AI gate");
        assert_eq!(receiver.try_recv().unwrap(),
            crate::auction_bridge::ActorEvent::BidVoiceGate(false));
    }

    #[test]
    #[ignore = "requires original v1.4 auction character assets"]
    fn original_auction_withdraw_notice_uses_third_voice_at_callback() {
        use openmt2001::ai_messages::offers::auction::{self, Command};
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let exe = read("OPENMT2001_EXE");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let image = PeImage::parse(&exe).unwrap();
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let assets = Assets::load(&image, &index, &archive).unwrap();
        let mut actors = Actors::from_original(&image, &assets, &[0, 1]).unwrap();

        // 464769 selects the third character pointer, 4647a5 releases the
        // retained handle, then 4647bc tests the panel's +858 sound gate.
        assert_eq!(image.at(0x46477d, 7).unwrap(), &[0x8b, 0x04, 0x95, 0xa0, 0xc2, 0x53, 0x00]);
        assert_eq!(image.at(0x4647bc, 10).unwrap(), &[0x8a, 0x82, 0x58, 0x08, 0, 0, 0x84, 0xc0, 0x75, 0x32]);
        assert_eq!(image.at(0x4647c6, 8).unwrap(), &[0x6a, 100, 0x6a, 1, 0x6a, 0, 0x6a, 0]);
        let call = image.at(0x4647da, 5).unwrap();
        assert_eq!(call[0], 0xe8);
        assert_eq!((0x4647dfu32).wrapping_add_signed(i32::from_le_bytes(call[1..5].try_into().unwrap())), 0x4ad760);

        let mut state = [0u8; 128];
        state[8] = 1;
        state[20] = 1;
        state[0x50..0x54].copy_from_slice(&u32::MAX.to_le_bytes());
        state[0x60..0x64].copy_from_slice(&u32::MAX.to_le_bytes());
        let (sender, receiver) = std::sync::mpsc::channel();
        let mut host_auction = Some(crate::live_host::Auction::default());
        let mut omitted = std::collections::BTreeSet::new();
        auction::apply(Command { action: 2, participant: 0, amount: 0 },
            &mut state, 2, true, |call, _| {
                crate::live_host::auction_action(call, &mut host_auction, &mut omitted, 0,
                    &Some(sender.clone()), &std::cell::Cell::new(false)).map(|_| ())
            }).unwrap();
        assert_eq!(receiver.try_iter().collect::<Vec<_>>(),
            vec![crate::auction_bridge::ActorEvent::WithdrawNotice(0)]);
        assert_eq!(actors.withdraw(0).unwrap(), ActorCue::Withdraw {
            participant: 0, character: 0, suppressed: false,
        });
        actors.set_bid_voice_suppressed(true);
        assert_eq!(actors.withdraw(0).unwrap(), ActorCue::Withdraw {
            participant: 0, character: 0, suppressed: true,
        });
        assert_eq!(actors.poses[7].frame, 1, "withdrawal has no actor-animation delay");
        assert!(!omitted.contains(&0x464769));
    }
    #[test]
    #[ignore = "requires original result character meshes and texture pages"]
    fn original_result_actor_resources() {
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let exe = read("OPENMT2001_EXE");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let image = PeImage::parse(&exe).unwrap();
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let assets = Assets::load_result(&image, &index, &archive).unwrap();
        for (winner, loser) in [(0, 1), (9, 8), (10, 10)] {
            let poses = result_layout((winner, loser)).unwrap();
            assert_eq!(poses.len(), if loser == 10 { 1 } else { 2 });
            let draws = assets.draws(&poses).unwrap();
            assert!(draws.iter().any(|draw| !draw.vertices.is_empty()));
        }
    }
    #[cfg(feature = "capture")]
    #[test]
    #[ignore = "requires original result meshes, camera track and GPU"]
    fn capture_original_result_actors_on_flyby() {
        let output = std::env::var("OPENMT2001_RESULT_ACTOR_CAPTURE")
            .expect("capture output directory");
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let exe = read("OPENMT2001_EXE");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let image = PeImage::parse(&exe).unwrap();
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let actors = Assets::load_result(&image, &index, &archive).unwrap();
        let installer = std::path::PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap())
            .parent().unwrap().parent().unwrap().to_path_buf();
        let track = std::fs::read(installer.join("Standard/Cut_scenes/win_cam.tm")).unwrap();
        let flyby = openmt2001::flyby::Flyby::parse(&track).unwrap();
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
        let renderer = pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
        std::fs::create_dir_all(&output).unwrap();
        for (winner, loser) in [(0, 1), (9, 8)] {
            let draws = actors.draws(&result_layout((winner, loser)).unwrap()).unwrap();
            for fraction in [1, 2, 3] {
                let frame = (flyby.frame_count() * fraction / 4).clamp(1, flyby.frame_count() - 1);
                let sample = flyby.sample(frame).unwrap();
                let [near, far] = openmt2001::camera::city_clip(sample.eye[1]).unwrap();
                let projection = openmt2001::camera::projection(
                    sample.vertical_fov, near, far, 1280, 960, [0., 1., 0., 1.]).unwrap();
                let view = openmt2001::camera::view_with_up(sample.eye, sample.target, sample.up);
                let matrix = (glam::Mat4::from_cols_array(&projection)
                    * glam::Mat4::from_cols_array(&view)).to_cols_array();
                let prepared = renderer.prepare(
                    &draws.iter().map(OwnedDraw::borrowed).collect::<Vec<_>>(), matrix, None).unwrap();
                let path = std::path::Path::new(&output)
                    .join(format!("result-actors-{winner}-{loser}-{fraction}.png"));
                crate::capture_test::capture_standalone(&renderer, &prepared, &path.to_string_lossy());
            }
        }
    }
    #[cfg(feature = "capture")]
    #[test]
    #[ignore = "requires original result meshes, SUNCOLS, flyby and GPU"]
    fn capture_original_result_actor_highlight() {
        let output =
            std::path::PathBuf::from(std::env::var("OPENMT2001_RESULT_ACTOR_CAPTURE").unwrap());
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let exe = read("OPENMT2001_EXE");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let image = PeImage::parse(&exe).unwrap();
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let actors = Assets::load_result(&image, &index, &archive).unwrap();
        let installer = std::path::PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap())
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let track = std::fs::read(installer.join("Standard/Cut_scenes/win_cam.tm")).unwrap();
        let flyby = openmt2001::flyby::Flyby::parse(&track).unwrap();
        // A fixed original daylight sample isolates the camera highlight;
        // this test does not claim the result is always entered at hour 12.
        let suncols = std::fs::read(installer.join("Standard/SUNCOLS.tga")).unwrap();
        let light = openmt2001::lighting::SunColors::parse(&suncols)
            .unwrap()
            .sample(12, 0)
            .unwrap();
        let poses = result_layout((0, 1)).unwrap();
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
        std::fs::create_dir_all(&output).unwrap();
        let frame = (flyby.frame_count() / 4).clamp(1, flyby.frame_count() - 1);
        let sample = flyby.sample(frame).unwrap();
        let baseline = actors.draws(&poses).unwrap();
        let lit = actors.draws_result(&poses, &light, sample.eye).unwrap();
        assert!(
            baseline
                .iter()
                .zip(&lit)
                .any(|(a, b)| a.specular != b.specular),
            "native result camera light made no actor vertex changes"
        );
        let [near, far] = openmt2001::camera::city_clip(sample.eye[1]).unwrap();
        let projection = openmt2001::camera::projection(
            sample.vertical_fov,
            near,
            far,
            1280,
            960,
            [0., 1., 0., 1.],
        )
        .unwrap();
        let view = openmt2001::camera::view_with_up(sample.eye, sample.target, sample.up);
        let matrix = (glam::Mat4::from_cols_array(&projection)
            * glam::Mat4::from_cols_array(&view))
        .to_cols_array();
        for (name, draws) in [("before", baseline), ("camera-lit", lit)] {
            let prepared = renderer
                .prepare(
                    &draws.iter().map(OwnedDraw::borrowed).collect::<Vec<_>>(),
                    matrix,
                    None,
                )
                .unwrap();
            let path = output.join(format!("result-actor-light-{name}.png"));
            crate::capture_test::capture_standalone(&renderer, &prepared, &path.to_string_lossy());
        }
    }
    #[test]
    #[ignore = "requires original auction scene observations"]
    fn original_auction_poses_and_camera() {
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_AUCTION_SCENE").unwrap()).unwrap();
        struct Reader<'a>(&'a [u8]);
        impl Reader<'_> {
            fn word(&mut self) -> u32 {
                let (a, b) = self.0.split_at(4);
                self.0 = b;
                u32::from_le_bytes(a.try_into().unwrap())
            }
            fn rows(&mut self) -> Vec<(u32, [u32; 4], u16)> {
                (0..self.word())
                    .map(|_| {
                        let model = self.word();
                        let pose = std::array::from_fn(|_| self.word());
                        let (a, b) = self.0.split_at(2);
                        self.0 = b;
                        (model, pose, u16::from_le_bytes(a.try_into().unwrap()))
                    })
                    .collect()
            }
        }
        assert_eq!(&data[..8], b"MTAUSC01");
        let mut r = Reader(&data[8..]);
        let count = r.word();
        let static_rows = r.rows();
        let compare = |actual: &Pose, expected: &(u32, [u32; 4], u16)| {
            let model = if actual.model < 4 {
                0x6883c0 + actual.model as u32 * 16
            } else {
                0x685bf8 + (actual.model as u32 - 4) * 16
            };
            assert_eq!(model, expected.0);
            assert_eq!(
                [
                    actual.position[0].to_bits(),
                    actual.position[1].to_bits(),
                    actual.position[2].to_bits(),
                    actual.yaw.to_bits()
                ],
                expected.1
            );
            assert_eq!(actual.flags, expected.2);
        };
        for _ in 0..count {
            let characters = (0..r.word()).map(|_| r.word()).collect::<Vec<_>>();
            let dynamic = r.rows();
            let layout = initial_layout(&characters).unwrap();
            assert_eq!(layout.len(), static_rows.len() + dynamic.len());
            for (a, b) in layout.iter().zip(static_rows.iter().chain(&dynamic)) {
                compare(a, b);
            }
        }
        let camera = initial_camera();
        for value in camera.eye.into_iter().chain(camera.target) {
            assert_eq!(value.to_bits(), r.word());
        }
        assert!(r.0.is_empty());
        println!("Original60 auction layouts, static records and camera bitwise matched");
    }

    #[test]
    #[ignore = "requires original auction meshes and texture pages"]
    fn original_auction_scene_resources() {
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let exe = read("OPENMT2001_EXE");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let image = PeImage::parse(&exe).unwrap();
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let assets = Assets::load(&image, &index, &archive).unwrap();
        // Every character must resolve its native idle,bid,winning endpoint.
        for character in 0..10 {
            let mut layout = initial_layout(&[character]).unwrap();
            for address in [0x53c584, 0x53c5d4, 0x53c624] {
                let end = u16::from_le_bytes(
                    image
                        .at(address + character * 4, 2)
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );
                layout[7].frame = usize::from(end) - 1;
                let draws = assets.draws(&layout).unwrap();
                assert!(
                    draws
                        .iter()
                        .all(|draw| draw.vertices.iter().flatten().all(|v| v.is_finite()))
                );
            }
        }
        #[cfg(feature = "capture")]
        if let Ok(path) = std::env::var("OPENMT2001_AUCTION_SCENE_CAPTURE") {
            let draws = assets
                .draws(&initial_layout(&[0, 2, 8, 9]).unwrap())
                .unwrap();
            let sample = initial_camera();
            let [near, far] = openmt2001::camera::city_clip(sample.eye[1]).unwrap();
            let projection = openmt2001::camera::projection(
                sample.vertical_fov,
                near,
                far,
                1280,
                960,
                [0., 1., 0., 1.],
            )
            .unwrap();
            let view = openmt2001::camera::view_with_up(sample.eye, sample.target, sample.up);
            let matrix = (glam::Mat4::from_cols_array(&projection)
                * glam::Mat4::from_cols_array(&view))
            .to_cols_array();
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(windows) {
                descriptor.backends = wgpu::Backends::DX12;
            }
            let renderer =
                pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
            let prepared = renderer
                .prepare(
                    &draws.iter().map(OwnedDraw::borrowed).collect::<Vec<_>>(),
                    matrix,
                    None,
                )
                .unwrap();
            crate::capture_test::capture_standalone(&renderer, &prepared, &path);
        }
    }
}
