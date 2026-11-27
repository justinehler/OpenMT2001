//! Portable normal-auction continuation. Host scene/audio are rebuilt first.
use super::{Input, Output, Session, put, word};
use crate::auction_controls::{State, TimerText, TurnTimer};

#[derive(Clone, Debug)]
pub struct AuctionHostState {
    pub controls: State,
    pub visible: bool,
    pub interface_ticks: u64,
    pub announcement_seconds: Option<i32>,
}
#[derive(Clone, Debug)]
pub struct AuctionContinuation {
    pub game_mode: u32,
    pub prior_mode: u32,
    pub host: Option<AuctionHostState>,
    record: [u8; 344],
    active_count: u32,
    participant_ui: [u32; 7],
    bid_ui: [u32; 7],
    opening: u8,
    visible: bool,
    selected: [u32; 2],
    announcement: [u32; 3],
    countdown: u32,
    countdown_last: i32,
    activation_player_flags: Vec<(u32, u8)>,
}
fn boolean(input: &mut Input<'_>) -> Result<bool, String> {
    match input.word()? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err("invalid V2 auction boolean".into()),
    }
}
fn optional(input: &mut Input<'_>) -> Result<Option<u32>, String> {
    Ok(if boolean(input)? {
        Some(input.word()?)
    } else {
        None
    })
}
fn write_optional(output: &mut Output, value: Option<u32>) {
    output.word(u32::from(value.is_some()));
    if let Some(value) = value {
        output.word(value);
    }
}
impl AuctionContinuation {
    pub(super) fn capture(
        session: &Session<'_>,
        game_mode: u32,
        host: Option<AuctionHostState>,
        prior_mode: u32,
    ) -> Result<Self, String> {
        Self::capture_state(
            &session.world.auction,
            session.tutorial_state,
            game_mode,
            host,
            prior_mode,
        )
    }
    pub(super) fn capture_state(
        auction: &openmt2001::ai_messages::offers::auction::State,
        tutorial_state: u8,
        game_mode: u32,
        host: Option<AuctionHostState>,
        prior_mode: u32,
    ) -> Result<Self, String> {
        let record = auction.controller.record;
        if auction.controller.reference == 0
            || !matches!(prior_mode, 0 | 3)
            || !matches!(game_mode, 1 | 2)
            || (game_mode == 2
                && (record[0x7c] == 0 || host.is_none() || prior_mode != word(&record, 0x150)))
            || (game_mode == 1
                && (record[0x7c] != 0
                    || host.as_ref().is_none_or(|h| h.visible)
                    || (record[0x7d] == 0 && auction.controller.active_count != 0)))
        {
            return Err(
                "Auction save requires a normal announcement, active rebuilt bidding scene, or settled normal controller"
                    .into(),
            );
        }
        if tutorial_state != 0 {
            return Err("Tutorial auction saving is unsupported".into());
        }
        let p = &auction.presentation;
        Ok(Self {
            game_mode,
            prior_mode,
            host,
            record,
            active_count: auction.controller.active_count,
            participant_ui: auction.participant_ui,
            bid_ui: auction.bid_ui,
            opening: auction.opening,
            visible: p.visible,
            selected: p.selected,
            announcement: p.announcement,
            countdown: p.countdown,
            countdown_last: p.countdown_last,
            activation_player_flags: Vec::new(),
        })
    }
    pub(super) fn encode(&self) -> Vec<u8> {
        let mut out = Output(Vec::new());
        for value in [
            self.game_mode,
            self.prior_mode,
            self.active_count,
            u32::from(self.opening),
            u32::from(self.visible),
            self.countdown,
            self.countdown_last as u32,
        ] {
            out.word(value);
        }
        out.0.extend_from_slice(&self.record);
        for value in self
            .participant_ui
            .into_iter()
            .chain(self.bid_ui)
            .chain(self.selected)
            .chain(self.announcement)
        {
            out.word(value);
        }
        out.word(u32::from(self.host.is_some()));
        if let Some(host) = &self.host {
            out.word(u32::from(host.visible));
            out.word(host.interface_ticks as u32);
            out.word((host.interface_ticks >> 32) as u32);
            write_optional(&mut out, host.announcement_seconds.map(|v| v as u32));
            let c = &host.controls;
            for value in [
                u32::from(c.buttons_enabled),
                c.timer.remaining,
                c.timer.last as u32,
                u32::from(c.timer_visible),
                c.turn_notice_count,
                u32::from(c.details),
                c.ticks,
                u32::from(c.blink),
                c.displayed_increment,
            ] {
                out.word(value);
            }
            match c.timer_text {
                None => out.word(0),
                Some(TimerText::Seconds(v)) => {
                    out.word(1);
                    out.word(v as u32);
                }
                Some(TimerText::Label { no_sale }) => {
                    out.word(2);
                    out.word(u32::from(no_sale));
                }
            }
            write_optional(&mut out, c.price_label);
        }
        out.0
    }
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut input = Input::new(bytes);
        let game_mode = input.word()?;
        let prior_mode = input.word()?;
        let active_count = input.word()?;
        let opening = u8::try_from(input.word()?).map_err(|_| "invalid V2 auction opening byte")?;
        let visible = boolean(&mut input)?;
        let countdown = input.word()?;
        let countdown_last = input.word()? as i32;
        let record: [u8; 344] = input.take(344)?.try_into().unwrap();
        let mut participant_ui = [0; 7];
        let mut bid_ui = [0; 7];
        let mut selected = [0; 2];
        let mut announcement = [0; 3];
        for value in participant_ui
            .iter_mut()
            .chain(bid_ui.iter_mut())
            .chain(selected.iter_mut())
            .chain(announcement.iter_mut())
        {
            *value = input.word()?;
        }
        let host = if boolean(&mut input)? {
            let visible = boolean(&mut input)?;
            let interface_ticks = u64::from(input.word()?) | (u64::from(input.word()?) << 32);
            let announcement_seconds = optional(&mut input)?.map(|v| v as i32);
            let buttons_enabled =
                u8::try_from(input.word()?).map_err(|_| "invalid V2 auction button byte")?;
            let timer = TurnTimer {
                remaining: input.word()?,
                last: input.word()? as i32,
                sound: 0,
            };
            let timer_visible = boolean(&mut input)?;
            let turn_notice_count = input.word()?;
            let details = boolean(&mut input)?;
            let ticks = input.word()?;
            let blink = boolean(&mut input)?;
            let displayed_increment = input.word()?;
            let timer_text = match input.word()? {
                0 => None,
                1 => Some(TimerText::Seconds(input.word()? as i32)),
                2 => Some(TimerText::Label {
                    no_sale: boolean(&mut input)?,
                }),
                _ => return Err("invalid V2 auction timer text".into()),
            };
            let price_label = optional(&mut input)?;
            Some(AuctionHostState {
                visible,
                interface_ticks,
                announcement_seconds,
                controls: State {
                    buttons_enabled,
                    timer,
                    timer_visible,
                    timer_text,
                    turn_notice_count,
                    details,
                    ticks,
                    blink,
                    displayed_increment,
                    price_label,
                },
            })
        } else {
            None
        };
        input.end()?;
        if !matches!(game_mode, 1 | 2)
            || !matches!(prior_mode, 0 | 3)
            || (game_mode == 2
                && (record[0x7c] == 0 || host.is_none() || prior_mode != word(&record, 0x150)))
            || (game_mode == 1
                && (record[0x7c] != 0
                    || host.as_ref().is_none_or(|h| h.visible)
                    || (record[0x7d] == 0 && active_count != 0)))
        {
            return Err("invalid V2 auction phase metadata".into());
        }
        Ok(Self {
            game_mode,
            prior_mode,
            host,
            record,
            active_count,
            participant_ui,
            bid_ui,
            opening,
            visible,
            selected,
            announcement,
            countdown,
            countdown_last,
            activation_player_flags: Vec::new(),
        })
    }
    // The native activation primitive only mutates AI runtime+1df8. Retain
    // its checked V2 value after section2 restoration, without repeating the
    // player stream or retaining any renderer/audio allocation.
    pub(super) fn retain_activation_players(&mut self, session: &Session<'_>) {
        self.activation_player_flags = session
            .roster
            .manager
            .players
            .iter()
            .filter(|p| word(&p.bytes, 0x154) == 1)
            .map(|p| (p.reference, p.bytes[0x1df8]))
            .collect();
    }
    /// Invoke after original activation has rebuilt the bidding scene and sound.
    /// Countdown mode1 has no scene yet, so the host's activation hook is a no-op.
    pub fn apply(&self, session: &mut Session<'_>) -> Result<(), String> {
        let dormant = self.record[0x7c] == 0 && self.record[0x7d] == 0;
        if session.saved_game_mode != Some(self.game_mode)
            || (!dormant && session.world.auction.controller.reference == 0)
        {
            return Err("V2 auction continuation has no checked native restored controller".into());
        }
        for (reference, flag) in &self.activation_player_flags {
            let player = session
                .roster
                .manager
                .players
                .iter_mut()
                .find(|p| p.reference == *reference)
                .ok_or("V2 auction activation player is absent")?;
            player.bytes[0x1df8] = *flag;
        }
        let auction = &mut session.world.city.auction;
        // Native writer omits a settled controller whose two phase flags are
        // clear. Its audited fixed object survives closure in live sessions.
        if dormant {
            auction.controller.reference = 0x7f4758;
        }
        let rebuilt_transition = word(&auction.controller.record, 0x154);
        auction.controller.record = self.record;
        if self.game_mode == 2 {
            put(&mut auction.controller.record, 0x154, rebuilt_transition);
        }
        auction.controller.active_count = self.active_count;
        auction.participant_ui = self.participant_ui;
        auction.bid_ui = self.bid_ui;
        auction.opening = self.opening;
        let p = &mut auction.presentation;
        p.visible = self.visible;
        p.selected = self.selected;
        p.announcement = self.announcement;
        p.countdown = self.countdown;
        p.countdown_last = self.countdown_last;
        Ok(())
    }
    pub(super) fn validate_native(
        &self,
        save: &openmt2001::save::SaveFile<'_>,
    ) -> Result<(), String> {
        let native = save.auction()?;
        let dormant = self.record[0x7c] == 0 && self.record[0x7d] == 0;
        if save.game_mode()? != self.game_mode
            || (dormant && (native.is_some() || self.active_count != 0))
            || (!dormant && native != Some(&self.record))
        {
            return Err("V2/native auction continuation records disagree".into());
        }
        Ok(())
    }
}
