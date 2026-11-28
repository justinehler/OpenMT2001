//! Borrow the real Intro preference cells for synchronous App-event commits.
//! Durable preferences are distinct from current/saved process focus audio.
use super::process_frontend::Owner;
use crate::{
    menu::Menu,
    profile_settings::Snapshot,
    settings_menu::{GameplayLevels, GraphicsLevels, Levels},
};
use std::path::PathBuf;

pub(super) struct Cells<'a> {
    pub profile: &'a mut Option<PathBuf>,
    pub saved: &'a mut Snapshot,
    pub error: &'a mut Option<String>,
    pub audio: &'a mut Levels,
    pub graphics: &'a mut GraphicsLevels,
    pub gameplay: &'a mut GameplayLevels,
}
impl Cells<'_> {
    pub(super) fn sync_profile(
        &mut self,
        menu: Option<&mut Menu>,
        process: Option<&Owner>,
    ) -> Result<(), String> {
        let selected = menu.as_ref().and_then(|menu| menu.current_settings_path());
        if selected == *self.profile {
            return Ok(());
        }
        let snapshot = match &selected {
            Some(path) => Snapshot::load(path)?,
            None => Snapshot::default(),
        };
        // An actual identity change is a writer even when durable bytes match.
        // This preserves SharedAudio saved fields and desired pause.
        if let Some(process) = process {
            process.borrow_mut().commit_levels(snapshot.audio);
        }
        *self.profile = selected;
        *self.saved = snapshot;
        *self.error = None;
        *self.audio = snapshot.audio;
        *self.graphics = snapshot.graphics;
        *self.gameplay = snapshot.gameplay;
        if let Some(menu) = menu {
            let board = match self.profile.as_ref() {
                Some(path) => crate::board_selection::Name::load(path)?,
                None => crate::board_selection::Name::initial(),
            };
            menu.settings.set_board(board);
            menu.settings_audio = snapshot.audio;
            menu.settings.graphics = snapshot.graphics;
            menu.settings.gameplay = snapshot.gameplay;
        }
        Ok(())
    }
    pub(super) fn commit_menu(
        &mut self,
        menu: &mut Menu,
        process: Option<&Owner>,
    ) -> Result<(), String> {
        self.sync_profile(Some(&mut *menu), process)?;
        if let Some(path) = menu.current_settings_path() {
            menu.settings.persist_board(&path)?;
        }
        let snapshot = menu.settings_snapshot();
        if *self.audio != snapshot.audio {
            if let Some(process) = process {
                process.borrow_mut().commit_levels(snapshot.audio);
            }
        }
        *self.audio = snapshot.audio;
        *self.graphics = snapshot.graphics;
        *self.gameplay = snapshot.gameplay;
        self.persist(snapshot);
        Ok(())
    }
    pub(super) fn persist(&mut self, snapshot: Snapshot) {
        if snapshot == *self.saved {
            return;
        }
        if let Some(path) = self.profile.as_ref() {
            match snapshot.save(path) {
                Ok(()) => {
                    *self.saved = snapshot;
                    *self.error = None;
                }
                Err(error) => {
                    if self.error.as_deref() != Some(&error) {
                        eprintln!("[Options] Could not save selected profile settings: {error}");
                    }
                    *self.error = Some(error);
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "menu_preferences_tests.rs"]
mod tests;
