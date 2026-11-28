//! Apply one accepted/profile board to all retained city name consumers.
use super::*;
impl Intro {
    pub(super) fn sync_board_names(&mut self) -> Result<(), String> {
        let Some(menu) = self.menu.as_ref() else {
            return Ok(());
        };
        let name = menu.settings.board;
        if self.inspection.is_some() || self.assets.board_name == name {
            return Ok(());
        }
        let inputs = &self.assets.inputs;
        let board = menu.settings.board_language(&inputs.installer)?;
        let image = openmt2001::pe::PeImage::parse(&inputs.executable)?;
        let style =
            crate::block_sidewalk_labels::load_board_style(&image, &inputs.installer, name)?;
        let index = openmt2001::archive::ArchiveIndex::parse(
            &inputs.directory,
            inputs.archive.len() as u64,
        )
        .map_err(|e| e.to_string())?;
        let sidewalk = crate::block_sidewalk_labels::Assets::load_board(
            &index,
            &inputs.archive,
            &inputs.installer,
            &self.assets.selection.blocks,
            &style,
            &board,
        )?;
        if let Some(menu) = self.menu.as_mut() {
            menu.apply_board(&board, &self.assets.selection.blocks)?;
        }
        self.assets
            .ui_text
            .install_board(&inputs.executable, &inputs.installer, name)?;
        self.assets.time_panel.apply_board(&board)?;
        self.assets.save_dialog.apply_board(&board)?;
        self.assets.tooltip.apply_board(&board);
        let (prepared, _) = sidewalk.prepare(&self.renderer, &self.assets.scene)?;
        self.sidewalk_labels = prepared;
        self.sidewalk_shading = Default::default();
        self.assets.sidewalk_labels = sidewalk;
        style.apply_blocks(&mut self.assets.selection.blocks)?;
        let inputs = Arc::make_mut(&mut self.assets.inputs);
        self.assets.board_name = name;
        inputs.style = style;
        Ok(())
    }
}
