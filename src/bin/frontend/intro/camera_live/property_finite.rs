//! Shared finite algorithms over actual typed owners. No synthetic State/world
//! is created for a constructor. Implementations are internal production views.
use super::super::case_zero_text::{Fonts, Text};

///41f120/41f150 native conditional442af0 and blocked333 store. Down335 stays.
pub(in super::super) fn reset_control(
    shown: &mut u8,
    mode: u32,
    selected: &mut u8,
    blocked: &mut u8,
) {
    *shown = 0;
    if mode == 1 {
        *selected = 0;
    }
    *blocked = 0;
}
///404940 report0. Real glyph effects on the one parent+4b8 Text;16 is palette.
pub(in super::super) fn report_zero_title(
    text: &mut Text,
    fonts: &mut Fonts,
    shown: &mut u8,
) -> Result<(), String> {
    text.reset(fonts)?;
    *shown = 1;
    text.identifier(fonts, 28)?;
    text.resize()?;
    for c in &mut text.colors[..text.group.count as usize] {
        *c = 16;
    }
    Ok(())
}
///Precise402c30 projection; each offset is a native field identity, not a key
///to a caller-supplied object map. Both views bind their actual concrete owners.
pub(in super::super) trait ResetView {
    fn flag10(&mut self, value: u8) -> Result<(), String>;
    fn infos_reset(&mut self) -> Result<(), String>;
    fn title_hide(&mut self) -> Result<(), String>;
    fn hide(&mut self, offset: u32) -> Result<(), String>;
    fn selector_hide(&mut self, offset: u32) -> Result<(), String>;
    fn active1c(&mut self, value: u8) -> Result<(), String>;
}
pub(in super::super) fn reset_owned(v: &mut impl ResetView) -> Result<(), String> {
    v.flag10(0)?;
    v.infos_reset()?;
    v.title_hide()?;
    for offset in [0x2c, 0x30, 0x64] {
        v.hide(offset)?;
    }
    v.selector_hide(0x68)?;
    v.hide(0x74)?;
    v.selector_hide(0x78)?;
    v.hide(0x80)?;
    v.selector_hide(0x84)?;
    v.hide(0xb0)?;
    v.active1c(0)?;
    for offset in [0xac, 0xbc, 0x8c] {
        v.hide(offset)?;
    }
    v.selector_hide(0x90)?;
    v.hide(0x6c)?;
    v.hide(0x38)
}
///402d10 repeats native stores after402c30 and resets parent notice. Kept as
///one algorithm shared with the finite live child, without reconstructing it.
pub(in super::super) trait ExitView: ResetView {
    fn notice_reset(&mut self) -> Result<(), String>;
}
pub(in super::super) fn exit_owned(v: &mut impl ExitView) -> Result<(), String> {
    reset_owned(v)?;
    v.notice_reset()?;
    for offset in [0x2c, 0x30, 0x80] {
        v.hide(offset)?;
    }
    v.selector_hide(0x84)?;
    v.hide(0xb0)?;
    v.active1c(0)?;
    for offset in [0xac, 0xbc, 0x8c] {
        v.hide(offset)?;
    }
    v.selector_hide(0x90)?;
    v.hide(0x6c)
}
///Source4055f0 tail /4063f0 common no-selection report0 sequence. The concrete
///views must independently sample their real selection source at every guard.
pub(in super::super) trait NoSelectionView: ExitView {
    fn info_visibility(&mut self) -> Result<(), String>;
    fn report_zero(&mut self) -> Result<(), String>;
    fn title_zero(&mut self) -> Result<(), String>;
    fn dispatch_zero(&mut self) -> Result<(), String>;
    fn select_info_zero(&mut self) -> Result<(), String>;
    fn child_title_reset(&mut self) -> Result<(), String>;
    fn mode_zero(&mut self) -> Result<(), String>;
    fn activate(&mut self) -> Result<(), String>;
}
pub(in super::super) fn no_selection_cycle(v: &mut impl NoSelectionView) -> Result<(), String> {
    reset_owned(v)?;
    v.info_visibility()?;
    v.report_zero()?;
    v.title_zero()?;
    v.dispatch_zero()?;
    v.select_info_zero()?;
    restore_no_selection_mode(v)
}
///402aa9..402af1 and402b0e..402b74, also the existing4055f0/4063f0
///mode0 suffix. Views resample actual selection, pending and typed list owners.
pub(in super::super) fn info_no_selection(v: &mut impl NoSelectionView) -> Result<(), String> {
    v.select_info_zero()?;
    v.info_visibility()?;
    v.title_zero()?;
    v.activate()
}
pub(in super::super) fn restore_no_selection_mode(
    v: &mut impl NoSelectionView,
) -> Result<(), String> {
    exit_owned(v)?;
    v.child_title_reset()?;
    v.mode_zero()?;
    info_no_selection(v)
}
