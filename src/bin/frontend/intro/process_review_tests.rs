//! Shared controlled-input witness resources. Actual SceneInputs and WAV
//!parsers; no full Assets::load (which would invoke the result decoder).
use super::*;
pub(crate) fn audio_view(root: PathBuf) -> (Inspection, process_frontend::Owner) {
    let inputs = camera_producer_live_tests::inputs(root);
    let image = openmt2001::pe::PeImage::parse(&inputs.executable).unwrap();
    let (loaded, session) = inputs
        .fresh_preview(
            camera_producer_live_tests::setup(&inputs),
            fresh_city_camera().0,
            &mut SnapshotCache::default(),
        )
        .unwrap();
    drop(session);
    let auction = crate::auction_audio::Assets::load(&image, &inputs.installer).unwrap();
    let transport = crate::transport_audio::Assets::load(&image, &inputs.installer).unwrap();
    let era = crate::era_audio::Assets::load(
        &image,
        &inputs.installer,
        std::path::Path::new("decoder-must-not-be-invoked"),
    )
    .unwrap();
    let sound = crate::gameplay_sound::Assets::load(&image, &inputs.installer).unwrap();
    let view = camera_live::host_primary_containing_tests::inspection_products(
        loaded.scene,
        [0.; 4],
        [0.; 4],
        [0.; 4],
        auction,
        transport,
        era,
        sound,
    );
    let (process, mut activity) =
        crate::live_host::process_runtime::startup(&inputs.executable, "").unwrap();
    let owner = process_frontend::ProcessFrontend::new(
        activity.take_frontend_startup().unwrap(),
        process,
        winit::window::WindowId::from(7),
        crate::settings_menu::Levels::default(),
    )
    .unwrap();
    owner.borrow_mut().observe(true).unwrap();
    (view, owner)
}
