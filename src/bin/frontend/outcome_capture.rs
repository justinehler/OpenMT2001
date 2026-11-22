//! Test-only original result-dialog resource captures.
pub fn capture(
    request: crate::prompt::Request,
    session: &super::Session<'_>,
    assets: &super::CommandAssets<'_, '_>,
) {
    let Ok(directory) = std::env::var("OPENMT2001_OUTCOME_CAPTURE") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory);
    std::fs::create_dir_all(&directory).unwrap();
    let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
    let archive = read("OPENMT2001_ARCHIVE_BIN");
    let index = openmt2001::archive::ArchiveIndex::parse(
        &read("OPENMT2001_ARCHIVE_DIR"),
        archive.len() as u64,
    )
    .unwrap();
    let parameters = std::path::PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
    let installer = parameters.parent().unwrap().parent().unwrap();
    let exe = std::path::PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
    let language = exe
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("US/language/usa");
    let lengths = std::fs::read(language.join("tycoon.off")).unwrap();
    let bytes = std::fs::read(language.join("tycoon.bin")).unwrap();
    let mut at = 0;
    let strings: Vec<_> = lengths
        .chunks_exact(2)
        .map(|b| {
            let end = at + usize::from(u16::from_le_bytes(b.try_into().unwrap()));
            let text = &bytes[at..end];
            at = end;
            text
        })
        .collect();
    let panel = crate::prompt::Assets::load(&index, &archive, installer, &strings).unwrap();
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    if cfg!(windows) {
        descriptor.backends = wgpu::Backends::DX12;
    }
    let renderer = pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
    let mut requests = vec![("live-result", request)];
    let local = session
        .roster
        .manager
        .players
        .iter()
        .position(|p| p.reference == session.roster.status.current_player)
        .unwrap();
    for (name, slot, kind) in [
        ("native-currency-win", local, 3),
        (
            "native-integer-rival",
            (local + 1) % session.roster.manager.players.len(),
            6,
        ),
    ] {
        let objective = openmt2001::scenario_objective::State {
            kind,
            target: 12500.75,
            ..Default::default()
        };
        let mut dialog = crate::human_dialog::Dialog::default();
        openmt2001::outcome_notice::present(
            slot as u32,
            2,
            &openmt2001::outcome_notice::World {
                image: assets.image,
                players: &session.roster.manager.players,
                current_reference: session.roster.status.current_player,
                objective: &objective,
            },
            |call| {
                if !matches!(
                    call,
                    openmt2001::outcome_notice::Call::Show
                        | openmt2001::outcome_notice::Call::Pause
                ) {
                    dialog.outcome(call)?;
                }
                Ok(())
            },
        )
        .unwrap();
        requests.push((
            name,
            crate::prompt::Request {
                dialog: Some(dialog),
                title: 0,
                body: 0,
                dismissal_id: 0,
                card: None,
            },
        ));
    }
    for (name, request) in requests {
        let mut state = crate::prompt::State::default();
        state.submit(request);
        let prepared = panel.prepare(&renderer, &state).unwrap();
        crate::capture_test::capture_standalone(
            &renderer,
            &prepared,
            &directory.join(format!("{name}.png")).to_string_lossy(),
        );
    }
}
