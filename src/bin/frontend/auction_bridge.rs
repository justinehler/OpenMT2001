//! Renderer side of the original auction scene boundary. The request protocol
//! lives in `openmt2001::render_requests::auction`.
use std::sync::mpsc;

pub use openmt2001::render_requests::auction::{ActorEvent, Operation, Request, request};

#[derive(Default)]
pub struct View {
    pub scene: Option<(openmt2001::render::PreparedScene, bool)>,
    actors: Option<ActorView>,
    cues: Vec<crate::auction_scene::ActorCue>,
    /// Original93b008; normal mode2 startup initializes0.
    transition: u32,
    saved_transition: Option<u32>,
}
struct ActorView {
    assets: crate::auction_scene::Assets,
    state: crate::auction_scene::Actors,
    events: mpsc::Receiver<ActorEvent>,
}
impl View {
    /// Drain the worker's producer stream in order. A draw refresh follows all
    /// frame advances in this UI pass, while cues retain their callback order.
    pub fn service_actors(
        &mut self,
        renderer: &openmt2001::render::Renderer,
    ) -> Result<(), String> {
        let Some(actors) = &mut self.actors else {
            return Ok(());
        };
        let events = actors.events.try_iter().collect::<Vec<_>>();
        let mut changed = false;
        for event in events {
            match event {
                ActorEvent::Effect { participant, kind } => {
                    actors.state.effect(participant, kind)?
                }
                ActorEvent::WithdrawNotice(participant) => {
                    self.cues.push(actors.state.withdraw(participant)?);
                }
                ActorEvent::BidVoiceGate(suppressed) => {
                    actors.state.set_bid_voice_suppressed(suppressed)
                }
                ActorEvent::Advance => {
                    self.cues.extend(actors.state.advance());
                    changed = true;
                }
            }
        }
        if changed {
            let scene = &mut self.scene.as_mut().ok_or("Auction actor scene missing")?.0;
            actors
                .assets
                .refresh(renderer, &actors.state.poses, scene)?;
        }
        Ok(())
    }

    pub fn take_cues(&mut self) -> Vec<crate::auction_scene::ActorCue> {
        std::mem::take(&mut self.cues)
    }

    pub fn present(
        &mut self,
        operation: Operation,
        renderer: &openmt2001::render::Renderer,
        source: (&[u8], &[u8], &[u8]),
        interface_mode: u32,
    ) -> Result<u32, String> {
        match operation {
            Operation::Prepare {
                characters,
                actor_events,
            } => {
                if self.scene.is_some() {
                    return Err("Auction renderer already owns an active scene".into());
                }
                let (exe, directory, archive) = source;
                let image = openmt2001::pe::PeImage::parse(exe)?;
                let index =
                    openmt2001::archive::ArchiveIndex::parse(directory, archive.len() as u64)
                        .map_err(|e| e.to_string())?;
                let assets = crate::auction_scene::Assets::load(&image, &index, archive)?;
                let state =
                    crate::auction_scene::Actors::from_original(&image, &assets, &characters)?;
                self.scene = Some((assets.prepare(renderer, &characters)?, false));
                self.actors = Some(ActorView {
                    assets,
                    state,
                    events: actor_events,
                });
                self.cues.clear();
                Ok(interface_mode)
            }
            Operation::Activate => {
                let scene = self
                    .scene
                    .as_mut()
                    .ok_or("Auction scene was not prepared")?;
                if self.saved_transition.is_some() {
                    return Err("Auction camera is already active".into());
                }
                scene.1 = true;
                let previous = std::mem::replace(&mut self.transition, 1);
                self.saved_transition = Some(previous);
                Ok(previous)
            }
            Operation::Restore => {
                let previous = self
                    .saved_transition
                    .take()
                    .ok_or("Auction restore has no retained camera state")?;
                self.scene
                    .take()
                    .ok_or("Auction restore has no retained scene")?;
                self.actors = None;
                self.cues.clear();
                self.transition = previous;
                Ok(0)
            }
        }
    }
}
