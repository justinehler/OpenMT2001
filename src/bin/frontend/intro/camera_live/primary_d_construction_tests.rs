//! Supplied-resource/actual World/protected operation regressions.
//! Original files are required through external paths; no successful provider.
use super::super::{main_construction as main, process_image::Cells};
use super::*;
use crate::live_host::camera_bridge::{
    self as bridge, Admission, Observation, Operation, Pose, SessionCall, SessionReply,
};
use std::{fs, path::PathBuf, thread, time::Duration};
struct Inputs {
    exe: Vec<u8>,
    archive: Vec<u8>,
    index: ArchiveIndex,
    installer: PathBuf,
    language: PathBuf,
    parameters: String,
    timetable: String,
}
fn path(key: &str) -> PathBuf {
    std::env::var_os(key)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("required {key}"))
}
impl Inputs {
    fn read() -> Self {
        let exe = fs::read(path("OPENMT2001_EXE")).unwrap();
        let archive = fs::read(path("OPENMT2001_ARCHIVE_BIN")).unwrap();
        let index = ArchiveIndex::parse(
            &fs::read(path("OPENMT2001_ARCHIVE_DIR")).unwrap(),
            archive.len() as u64,
        )
        .unwrap();
        Self {
            exe,
            archive,
            index,
            installer: path("OPENMT2001_INSTALLER"),
            language: path("OPENMT2001_LANGUAGE"),
            parameters: fs::read_to_string(path("OPENMT2001_PARAMETERS")).unwrap(),
            timetable: fs::read_to_string(path("OPENMT2001_TIMETABLE")).unwrap(),
        }
    }
    fn text(&self) -> UiTextResources {
        let mut text = UiTextResources::load(
            &self.exe,
            &self.installer,
            &self.language,
            &self.index,
            &self.archive,
        )
        .unwrap();
        text.install_usa_board(&self.exe, &self.installer).unwrap();
        text
    }
    fn primitives(&self) -> Primitives {
        let c = Cells::process_allocation(&self.exe).unwrap();
        let name = registration::primary_asset(&c, 0).unwrap();
        let e = self.index.find(&name).unwrap();
        let bytes = &self.archive[e.offset as usize..(e.offset + e.length) as usize];
        Primitives::navigator(
            &self.exe,
            CpuModel::parsed(PanelId::F, e, &CachedMesh::parse(bytes).unwrap()).unwrap(),
        )
        .unwrap()
    }
    fn images(&self) -> Images {
        Images::load(&self.exe, &self.index, &self.archive, [640, 480]).unwrap()
    }
    fn prepared(&self) -> (PrimaryConstruction, Primitives, UiTextResources, Images) {
        let mut p = self.primitives();
        let text = self.text();
        let images = self.images();
        let currency = PrimaryConstruction::initialize_currency(&mut p, &text).unwrap();
        let d = PrimaryConstruction::prepare(
            &self.exe,
            &self.index,
            &self.archive,
            currency,
            &mut p,
            &text,
            &images,
        )
        .unwrap();
        assert_eq!(d.phase(), PrimaryPhase::AwaitingDLayout);
        (d, p, text, images)
    }
    fn world(&self) -> openmt2001::level_startup::World {
        let pe = PeImage::parse(&self.exe).unwrap();
        let board = openmt2001::board::Board::from_v14(&pe).unwrap();
        let configuration = openmt2001::resident::Configuration::before_defaults(
            &self.parameters,
            &self.timetable,
            &pe,
        )
        .unwrap();
        //Explicit fixture creation policy; not Clock's5904f0 adoption or reseed.
        let creation = openmt2001::economy::ResidentCreationState {
            auxiliary: openmt2001::resident::AuxiliaryPool::for_residents(),
            initialization_rng: openmt2001::resident::InitializationRng {
                constructor: 7,
                movement: 11,
                objective: 13,
            },
            identity_rng: 17,
            preference_rng: 19,
        };
        openmt2001::level_startup::World::fresh(&pe, &board, configuration, creation).unwrap()
    }
}
fn install() -> Operation {
    Operation::Install {
        property: None,
        capture: openmt2001::camera::bank::Capture::from_records([[0; 196]; 2]),
        admission: Admission::default(),
    }
}
fn observation() -> Observation {
    Observation {
        raw_capture: None,
        current: Pose {
            eye: [0; 3],
            target: [0; 3],
            up: [0; 3],
        },
        sampled: None,
        strategic_writeback: None,
        finalization_fault: None,
        task_effects: vec![],
    }
}
fn row(
    world: &mut openmt2001::level_startup::World,
    i: usize,
    active: u8,
    excluded: u8,
    title: i32,
) {
    world.categories[i][4] = active;
    world.categories[i][0x90] = excluded;
    world.categories[i][0x8c..0x90].copy_from_slice(&title.to_le_bytes());
}
///Production read body on actual World, over actual protected same-operation wire.
///This is a component receipt, not a real camera/Graph/native-order receipt.
fn run(
    mut d: PrimaryConstruction,
    mut p: Primitives,
    mut text: UiTextResources,
    images: Images,
    world: &openmt2001::level_startup::World,
) -> (
    PrimaryConstruction,
    Primitives,
    UiTextResources,
    Images,
    Result<InstallDisposition, String>,
) {
    let source = ResidentSource::from_world(world);
    let generation = source.epoch().serial();
    let sent = source.clone();
    let (mut worker, mut inbox) = bridge::channel(generation);
    let ui = thread::spawn(move || {
        let mut outcome = None;
        let result = inbox.wait_service_with_handler(Duration::from_secs(30), |_, port| {
            let result = d.install(&sent, generation, port, &mut p, &mut text, &images);
            outcome = Some(result.clone());
            result?;
            Ok(observation())
        });
        let value = outcome.unwrap();
        assert_eq!(result.is_ok(), value.is_ok());
        inbox.retire();
        (d, p, text, images, value)
    });
    let outcome = worker.request_with_handler(install(), |call| match call {
        SessionCall::PrimaryCategory { source, row } => {
            crate::live_host::test_primary_category_read(world, &source, row)
        }
        _ => panic!("D install reached a mutating/general callback"),
    });
    let result = ui.join().unwrap();
    assert_eq!(outcome.is_ok(), result.4.is_ok());
    result
}
#[test]
#[ignore = "supplied PE/archive/board language/fonts/parameters"]
fn actual_d_static_pool_descriptor_currency_and_missing_source_stop() {
    let input = Inputs::read();
    let (d, p, text, images) = input.prepared();
    let c = p.cells();
    assert_eq!(p.primary_pool().unwrap().len(), 16);
    assert_eq!(c.word(G + 0x2c).unwrap(), 16);
    assert_eq!(c.word(G + 0x30).unwrap(), 16);
    for (ordinal, &id) in p.primary_pool().unwrap().iter().enumerate() {
        let descriptor = p.resources.descriptor(id).unwrap();
        let model = p.resources.model(id).unwrap();
        assert_eq!(descriptor.kind, 11);
        assert_eq!(descriptor.angle_x, 180.);
        assert_eq!(descriptor.definition, model.definition);
        let name = registration::primary_asset(c, ordinal as u8).unwrap();
        let e = input.index.find(&name).unwrap();
        assert_eq!(model.source.metadata, e.metadata);
    }
    assert_ne!(
        p.resources.model(PanelId::A).unwrap().frames.as_ptr(),
        p.resources.model(PanelId::G).unwrap().frames.as_ptr()
    );
    assert_eq!(c.word(D).unwrap(), 0x5255f0);
    assert_eq!(c.word(D + 0x388).unwrap(), 0);
    assert_eq!(
        c.word(D + 0x37c).unwrap(),
        0,
        "raw descriptor word is not a fabricated pointer"
    );
    assert_eq!(p.panels[&D].descriptor, Some(DescriptorIdentity::EmbeddedD));
    assert_eq!(c.byte(D + 0x184).unwrap(), 0);
    for o in [0x188, 0x4ac, 0xa7c, 0xc34] {
        assert_eq!(p.texts[&(D + o)].font, 0);
        assert_eq!(p.texts[&(D + o)].pitch, 10);
    }
    assert_eq!(c.word(G + 8).unwrap(), 0);
    assert_eq!(c.word(G + 12).unwrap(), 0);
    assert_eq!(c.word(G + 0x24).unwrap(), 16);
    assert_eq!(
        c.ntbs(0x57e6ec, 0x24).unwrap(),
        d.currency().symbol[..d.currency().symbol.len() - 1]
    );
    assert_eq!(c.word(0x59050c).unwrap(), d.currency().suffix);
    assert_eq!(d.phase(), PrimaryPhase::AwaitingDLayout);
    assert!(d.d.rows.is_empty());
    assert!(d.require_ordered_main().is_err());
    assert!(p.require_process_graph().is_err());
    drop((d, p, text, images));
}
#[test]
#[ignore = "supplied real World/resources"]
fn actual_world_admission_title45_46_forced41_and_only_d_registration() {
    let input = Inputs::read();
    let (d, p, text, images) = input.prepared();
    let mut world = input.world();
    for i in 0..52 {
        row(&mut world, i, 0, 1, 46);
    } //controlled SAME World fixture writes
    row(&mut world, 1, 1, 0, 45);
    row(&mut world, 2, 1, 1, 44);
    row(&mut world, 3, 1, 0, 46);
    row(&mut world, 41, 0, 1, 0);
    let fonts = text.fonts() as *const _;
    let f = p.resources.model(PanelId::F).unwrap().frames.as_ptr();
    let root = &*d.d as *const DProduct;
    let (d, p, text, _, result) = run(d, p, text, images, &world);
    result.unwrap();
    assert_eq!(&*d.d as *const DProduct, root);
    assert_eq!(text.fonts() as *const _, fonts);
    assert_eq!(p.resources.model(PanelId::F).unwrap().frames.as_ptr(), f);
    assert_eq!(d.phase(), PrimaryPhase::AwaitingVoting);
    assert_eq!(
        d.d.rows
            .iter()
            .map(|r| r.category.unwrap())
            .collect::<Vec<_>>(),
        vec![1, 41]
    );
    for (index, r) in d.d.rows.iter().enumerate() {
        assert_eq!(r.number, index as u32);
        assert_eq!(r.parent, D);
        assert_eq!(r.root, D);
        assert_eq!(r.text.bounds, [0., 0., 200., 40.]);
        assert_eq!(r.text.font, 0);
        assert!(r.initialized);
    }
    let c = p.cells();
    assert_eq!(c.byte(D + 0x184).unwrap(), 1);
    assert_eq!(c.word(G + 8).unwrap(), D);
    assert_eq!(c.word(G + 12).unwrap(), D);
    assert_eq!(c.word(G + 0x24).unwrap(), 17);
    assert_eq!(c.word(D + 0x380).unwrap(), 0);
    assert_eq!(c.word(D + 0x384).unwrap(), 0);
    assert_eq!(c.word(D + 0x4a4).unwrap(), 2);
    assert_eq!(c.word(D + 0xdf4).unwrap(), 91);
    for (at, value) in GEOMETRY {
        assert_eq!(c.word(at).unwrap(), value)
    }
    assert_eq!(
        p.resources
            .model(PanelId::OtherPrimary(1))
            .unwrap()
            .source
            .panel,
        PanelId::OtherPrimary(1)
    );
    assert!(d.require_ordered_main().is_err());
}
#[test]
#[ignore = "actual World/Fonts reached fault and identity"]
fn admitted_negative_title_latches_after_text_append_without_bind_or_register() {
    let input = Inputs::read();
    let (d, p, text, images) = input.prepared();
    let mut world = input.world();
    for i in 0..52 {
        row(&mut world, i, 0, 1, 46);
    }
    row(&mut world, 0, 1, 0, -1);
    let (d, p, text, images, result) = run(d, p, text, images, &world);
    assert!(result.unwrap_err().contains("signed before-table"));
    assert_eq!(d.phase(), PrimaryPhase::Failed);
    assert_eq!(d.d.rows.len(), 1);
    assert!(!d.d.rows[0].initialized);
    assert_eq!(p.cells().byte(D + 0x184).unwrap(), 0);
    assert_eq!(p.cells().word(G + 8).unwrap(), 0);
    assert_eq!(p.cells().word(D + 0x4a4).unwrap(), 0);
    let same = &*d.d.rows[0] as *const LegendText;
    row(&mut world, 0, 1, 0, 0);
    let (d, p, _, _, result) = run(d, p, text, images, &world);
    assert!(result.is_err());
    assert_eq!(&*d.d.rows[0] as *const LegendText, same);
    assert_eq!(p.cells().byte(D + 0x184).unwrap(), 0);
}
#[test]
#[ignore = "actual World/resource identities and preclaim retries"]
fn wrong_resource_preclaim_preserves_same_staged_d_and_fonts() {
    let input = Inputs::read();
    let (d, p, text, _) = input.prepared();
    let foreign = input.images();
    let world = input.world();
    let before = &*d.d as *const DProduct;
    let scratch = text.fonts().scratch.clone();
    let (d, p, text, _, result) = run(d, p, text, foreign, &world);
    assert!(result.unwrap_err().contains("SAME process/Fonts/images"));
    assert_eq!(d.phase(), PrimaryPhase::AwaitingDLayout);
    assert_eq!(&*d.d as *const DProduct, before);
    assert_eq!(text.fonts().scratch, scratch);
    assert_eq!(p.cells().byte(D + 0x184).unwrap(), 0);
}
#[test]
#[ignore = "actual World/operation stream protected read invariants"]
fn category_reader_observes_live_same_world_rejects_foreign_and_stale_origins() {
    let input = Inputs::read();
    let mut world = input.world();
    let other = input.world();
    let source = ResidentSource::from_world(&world);
    row(&mut world, 3, 1, 0, 45);
    assert_eq!(
        crate::live_host::test_primary_category_read(&world, &source, Some(3)).unwrap(),
        SessionReply::PrimaryCategory(Some([1, 0, 45]))
    );
    row(&mut world, 3, 2, 1, -7);
    assert_eq!(
        crate::live_host::test_primary_category_read(&world, &source, Some(3)).unwrap(),
        SessionReply::PrimaryCategory(Some([2, 1, (-7i32) as u32]))
    );
    assert!(crate::live_host::test_primary_category_read(&other, &source, None).is_err());
    assert!(crate::live_host::test_primary_category_read(&world, &source, Some(52)).is_err());
    world.creation.auxiliary.reset_for_residents();
    assert!(crate::live_host::test_primary_category_read(&world, &source, None).is_err());
}
#[test]
#[ignore = "supplied full existing Main construction chain and actual source owners"]
fn currency_precedes_main_helper_and_completed_main_f_survive_primary_prepare() {
    let input = Inputs::read();
    let mut p = input.primitives();
    let mut text = input.text();
    let mut images = input.images();
    let currency = PrimaryConstruction::initialize_currency(&mut p, &text).unwrap();
    let mut prefix = Some(
        main::Prefix::load(
            &input.exe,
            &input.index,
            &input.archive,
            [640, 480],
            &mut text,
            &mut p,
            &mut images,
        )
        .unwrap(),
    );
    let mut five = Some(
        main::AwaitingChild5::continue_child1(
            &mut prefix,
            &input.exe,
            &mut text,
            &mut p,
            &mut images,
        )
        .unwrap(),
    );
    let mut business = Some(
        main::AwaitingChild2::continue_child5(
            &mut five,
            &input.exe,
            &mut text,
            &mut p,
            &mut images,
        )
        .unwrap(),
    );
    let mut tail = Some(
        main::AwaitingParentTail::continue_child2(
            &mut business,
            &input.exe,
            &mut text,
            &mut p,
            &mut images,
        )
        .unwrap(),
    );
    let completed = main::CompletedMainLayout::continue_parent_tail(
        &mut tail,
        &input.exe,
        &mut text,
        &mut p,
        &mut images,
    )
    .unwrap();
    let root = completed.root();
    let nodes = completed
        .nodes()
        .map(|n| (n.id, n as *const main::Node))
        .collect::<Vec<_>>();
    let a = p.resources.model(PanelId::A).unwrap().frames.as_ptr();
    let f = p.resources.model(PanelId::F).unwrap().frames.as_ptr();
    let d = PrimaryConstruction::prepare(
        &input.exe,
        &input.index,
        &input.archive,
        currency,
        &mut p,
        &text,
        &images,
    )
    .unwrap();
    assert_eq!(completed.root(), root);
    assert_eq!(
        completed
            .nodes()
            .map(|n| (n.id, n as *const main::Node))
            .collect::<Vec<_>>(),
        nodes
    );
    assert_eq!(p.resources.model(PanelId::A).unwrap().frames.as_ptr(), a);
    assert_eq!(p.resources.model(PanelId::F).unwrap().frames.as_ptr(), f);
    assert_eq!(p.panels[&0x58cdb0].phase, Phase::Bound);
    assert_eq!(p.cells().byte(0x58e3c0 + 0x184).unwrap(), 0);
    assert_eq!(d.ordering_boundary(), OrderingBoundary::AwaitingOrderedMain);
    assert!(completed.require_full_main().is_err());
}
#[test]
#[ignore = "actual supplied text fault boundary and stable typed partial tree"]
fn reached_missing_font_keeps_partial_row_and_cannot_replay_after_repair() {
    let input = Inputs::read();
    let (d, p, mut text, images) = input.prepared();
    let mut world = input.world();
    for i in 0..52 {
        row(&mut world, i, 0, 1, 46);
    }
    row(&mut world, 1, 1, 0, 0);
    let font = text.fonts_mut().fonts.remove(&0).unwrap();
    let (d, p, mut text, images, result) = run(d, p, text, images, &world);
    assert!(result.is_err());
    assert_eq!(d.phase(), PrimaryPhase::Failed);
    assert_eq!(d.d.rows.len(), 1);
    assert!(!d.d.rows[0].initialized);
    let node = &*d.d.rows[0] as *const LegendText;
    text.fonts_mut().fonts.insert(0, font);
    let (d, p, _, _, result) = run(d, p, text, images, &world);
    assert!(result.is_err());
    assert_eq!(&*d.d.rows[0] as *const LegendText, node);
    assert_eq!(p.cells().byte(D + 0x184).unwrap(), 0);
    assert_eq!(p.cells().word(G + 8).unwrap(), 0);
}
#[test]
#[ignore = "actual World/operation wire live reread and install admission"]
fn install_category_reads_same_world_after_writer_and_rejects_other_callbacks() {
    let input = Inputs::read();
    let mut world = input.world();
    let source = ResidentSource::from_world(&world);
    let sent = source.clone();
    let generation = source.epoch().serial();
    row(&mut world, 3, 1, 0, 45);
    let (mut worker, mut inbox) = bridge::channel(generation);
    let ui = thread::spawn(move || {
        inbox
            .wait_service_with_handler(Duration::from_secs(30), |_, port| {
                let call = || SessionCall::PrimaryCategory {
                    source: sent.clone(),
                    row: Some(3),
                };
                assert_eq!(
                    port.call(call()).unwrap(),
                    SessionReply::PrimaryCategory(Some([1, 0, 45]))
                );
                assert_eq!(
                    port.call(call()).unwrap(),
                    SessionReply::PrimaryCategory(Some([2, 1, 44]))
                );
                //No mutating/general SourceProvider service may escape on Install.
                assert!(port.pause4bac90(bridge::PauseReason::Action3).is_err());
                Ok(observation())
            })
            .unwrap_err()
    });
    let mut reads = 0;
    let result = worker.request_with_handler(install(), |call| {
        let SessionCall::PrimaryCategory { source, row: which } = call else {
            panic!("forbidden install callback reached actual Host handler")
        };
        let value = crate::live_host::test_primary_category_read(&world, &source, which)?;
        reads += 1;
        if reads == 1 {
            row(&mut world, 3, 2, 1, 44);
        }
        Ok(value)
    });
    assert!(result.is_err());
    assert_eq!(reads, 2);
    assert!(ui.join().unwrap().contains("outside admitted central"));
}
#[test]
#[ignore = "supplied actual process descriptor reset/retained field semantics"]
fn same_embedded_descriptor_reset_preserves_unwritten_flags_and_definition() {
    let input = Inputs::read();
    let (_d, mut p, _, _) = input.prepared();
    p.primary_claim_d().unwrap();
    let descriptor = D + 0x388;
    {
        let c = p.primary_cells().unwrap();
        c.set_byte(descriptor + 0xb8, 0xf2).unwrap();
        c.write(descriptor + 0x32, &[0xaa, 0xbb]).unwrap();
        c.write(descriptor + 0x44, &[0x7d; 116]).unwrap();
    }
    p.primary_reset_d_descriptor().unwrap();
    let c = p.cells();
    assert_eq!(c.byte(descriptor + 0xb8).unwrap(), 0xfd);
    assert_eq!(c.read(descriptor + 0x32, 2).unwrap(), &[0xaa, 0xbb]);
    assert_eq!(c.read(descriptor + 0x44, 116).unwrap(), &[0x7d; 116]);
    assert_eq!(c.word(descriptor).unwrap(), 0);
    assert_eq!(c.half(descriptor + 0x2c).unwrap(), 0);
    assert_eq!(c.half(descriptor + 0x2e).unwrap(), u16::MAX);
    assert_eq!(
        c.read(descriptor + 0xbc, 48).unwrap(),
        c.read(0x559730, 48).unwrap()
    );
    assert_eq!(
        c.read(descriptor + 0xec, 48).unwrap(),
        c.read(0x559730, 48).unwrap()
    );
    assert_eq!(p.panels[&D].descriptor, Some(DescriptorIdentity::EmbeddedD));
    assert_eq!(c.byte(D + 0x184).unwrap(), 0);
    assert_eq!(c.word(G + 8).unwrap(), 0);
}
#[test]
#[ignore = "two supplied actual Worlds, actual readonly Install wire, retained Main/F"]
fn camera_only_two_installs_retire_and_reinstall_park_same_d_main_f_without_replay() {
    let input = Inputs::read();
    let mut p = input.primitives();
    let mut text = input.text();
    let mut images = input.images();
    let currency = PrimaryConstruction::initialize_currency(&mut p, &text).unwrap();
    let mut prefix = Some(
        main::Prefix::load(
            &input.exe,
            &input.index,
            &input.archive,
            [640, 480],
            &mut text,
            &mut p,
            &mut images,
        )
        .unwrap(),
    );
    let mut five = Some(
        main::AwaitingChild5::continue_child1(
            &mut prefix,
            &input.exe,
            &mut text,
            &mut p,
            &mut images,
        )
        .unwrap(),
    );
    let mut business = Some(
        main::AwaitingChild2::continue_child5(
            &mut five,
            &input.exe,
            &mut text,
            &mut p,
            &mut images,
        )
        .unwrap(),
    );
    let mut tail = Some(
        main::AwaitingParentTail::continue_child2(
            &mut business,
            &input.exe,
            &mut text,
            &mut p,
            &mut images,
        )
        .unwrap(),
    );
    let completed = main::CompletedMainLayout::continue_parent_tail(
        &mut tail,
        &input.exe,
        &mut text,
        &mut p,
        &mut images,
    )
    .unwrap();
    let main_nodes = completed
        .nodes()
        .map(|n| (n.id, n as *const main::Node))
        .collect::<Vec<_>>();
    let d = PrimaryConstruction::prepare(
        &input.exe,
        &input.index,
        &input.archive,
        currency,
        &mut p,
        &text,
        &images,
    )
    .unwrap();
    let mut one = input.world();
    let mut two = input.world();
    for i in 0..52 {
        row(&mut one, i, 0, 1, 46);
        row(&mut two, i, 0, 1, 46);
    }
    row(&mut one, 41, 0, 1, 0);
    row(&mut two, 41, 0, 1, 45);
    let epoch_one = one.topology.epoch().serial();
    let epoch_two = two.topology.epoch().serial();
    let (mut d, mut p, text, images, first) = run(d, p, text, images, &one);
    assert_eq!(
        first.unwrap(),
        InstallDisposition::CompletedD { epoch: epoch_one }
    );
    assert!(d.d_current_for(epoch_one));
    let d_root = &*d.d as *const DProduct;
    let rows =
        d.d.rows
            .iter()
            .map(|r| (r.id, &**r as *const LegendText, r.title))
            .collect::<Vec<_>>();
    let fonts = text.fonts() as *const _;
    let scratch = text.fonts().scratch.clone();
    let f = p.resources.model(PanelId::F).unwrap().frames.as_ptr();
    let a = p.resources.model(PanelId::A).unwrap().frames.as_ptr();
    let group = p.cells().read(G, 0x88).unwrap().to_vec();
    let d_bytes = p.cells().read(D, 0xdf8).unwrap().to_vec();
    let steps = p.steps.clone();
    d.retire_for_camera(&mut p).unwrap();
    assert!(!d.d_current_for(epoch_one));
    assert_eq!(d.phase(), PrimaryPhase::AwaitingDRebind);
    //run explicitly retired the first stream. SAME process products travel to
    //the second independent CameraOnly Install, with a distinct actual World.
    let (d, p, text, _, second) = run(d, p, text, images, &two);
    assert_eq!(
        second.unwrap(),
        InstallDisposition::ParkedAwaitingDRebind {
            constructed_epoch: epoch_one,
            requested_epoch: epoch_two
        }
    );
    assert_eq!(d.phase(), PrimaryPhase::AwaitingDRebind);
    assert_eq!(p.panels[&D].phase, Phase::Parked);
    assert_eq!(d.d.active_epoch, None);
    assert_eq!(d.d.parked_for, Some(epoch_two));
    assert!(!d.d_current_for(epoch_one));
    assert!(!d.d_current_for(epoch_two));
    assert!(d.require_current_d(epoch_two).is_err());
    assert!(d.require_ordered_main().is_err());
    assert!(p.require_process_graph().is_err());
    assert_eq!(&*d.d as *const DProduct, d_root);
    assert_eq!(
        d.d.rows
            .iter()
            .map(|r| (r.id, &**r as *const LegendText, r.title))
            .collect::<Vec<_>>(),
        rows
    );
    assert_eq!(p.cells().read(G, 0x88).unwrap(), group);
    assert_eq!(p.cells().read(D, 0xdf8).unwrap(), d_bytes);
    assert_eq!(p.steps, steps);
    assert_eq!(text.fonts() as *const _, fonts);
    assert_eq!(text.fonts().scratch, scratch);
    assert_eq!(p.resources.model(PanelId::F).unwrap().frames.as_ptr(), f);
    assert_eq!(p.resources.model(PanelId::A).unwrap().frames.as_ptr(), a);
    assert_eq!(
        completed
            .nodes()
            .map(|n| (n.id, n as *const main::Node))
            .collect::<Vec<_>>(),
        main_nodes
    );
    assert_eq!(p.cells().byte(0x58e3c0 + 0x184).unwrap(), 0);
    assert_eq!(p.panels[&0x58cdb0].phase, Phase::Bound);
}
