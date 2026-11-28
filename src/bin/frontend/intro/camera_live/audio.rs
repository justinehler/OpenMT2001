//! Existing available audio consumers extracted from exact public Intro.
//! Nonblocking drains only. No new source generation, 3D listener or RNG.
use super::super::Inspection;
use std::time::Instant;

/// Draw-owned audio service. Request drains also poll decoding, but only this
/// once-per-draw entry advances the outgoing music level.
pub(crate) fn service_frame(view: &mut Inspection, allow_music_poll: bool) -> Result<(), String> {
    service_with_music_poll(view, allow_music_poll)?;
    if music_admitted(view, allow_music_poll)
        && let Err(error) = view.era_audio_player.service()
    {
        eprintln!("Era music unavailable: {error}");
    }
    Ok(())
}
fn music_admitted(view: &Inspection, allow_music_poll: bool) -> bool {
    allow_music_poll
        && view
            .music_admission
            .as_ref()
            .is_none_or(|gate| gate.permits_attachment())
}

pub fn service_available(view: &mut Inspection) -> Result<(), String> {
    service_with_music_poll(view, true)
}
pub(crate) fn service_with_music_poll(
    view: &mut Inspection,
    allow_music_poll: bool,
) -> Result<(), String> {
    while let Some(request) = view
        .auction_audio_requests
        .as_ref()
        .and_then(|receiver| receiver.try_recv().ok())
    {
        let _ = request
            .reply
            .send(view.auction_audio_player.apply(request.operation));
    }
    view.transport_audio_player
        .set_interface_mode(if view.strategic { 3 } else { 0 });
    while let Some(request) = view
        .transport_audio_requests
        .as_ref()
        .and_then(|receiver| receiver.try_recv().ok())
    {
        view.transport_audio_player.apply(request.operation);
    }
    while let Some(request) = view
        .era_audio_requests
        .as_ref()
        .and_then(|receiver| receiver.try_recv().ok())
    {
        let _ = request
            .reply
            .send(view.era_audio_player.apply(request.operation));
    }
    let admitted = music_admitted(view, allow_music_poll);
    if admitted && let Err(error) = view.era_audio_player.poll() {
        eprintln!("Era music unavailable: {error}");
    }
    while let Some(request) = view
        .gameplay_sound_requests
        .as_ref()
        .and_then(|receiver| receiver.try_recv().ok())
    {
        if matches!(
            request.operation,
            crate::gameplay_sound::Operation::Ledger { .. }
        ) {
            view.last_status_query = Instant::now() - std::time::Duration::from_secs(1);
        }
        let result = view.gameplay_sound_player.apply(request.operation);
        if request.reply.send(result.clone()).is_err()
            && let Err(error) = result
        {
            eprintln!("Script sound unavailable: {error}");
        }
    }
    Ok(())
}
