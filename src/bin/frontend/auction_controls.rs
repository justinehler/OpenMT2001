//! Original414730 auction button handling. The controller record and current
//! player remain authoritative session inputs; this owns only interface state.
use openmt2001::ai_messages::offers::auction::Command;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Call {
    Affordable(f32),
    Action(Command),
    HideTimer,
    Tutorial(u32),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimerCall {
    ReleaseSound(u32),
    CreateSound,
    ClearText,
    WriteSeconds(i32),
    Label { no_sale: bool },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimerText {
    Seconds(i32),
    Label { no_sale: bool },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TurnTimer {
    pub remaining: u32,
    pub sound: u32,
    pub last: i32,
}
impl Default for TurnTimer {
    fn default() -> Self {
        Self {
            remaining: 800,
            sound: 0,
            last: -1,
        }
    }
}
impl TurnTimer {
    /// Original415ca0 message1, dispatched only while its panel is visible.
    /// This timer has no pause-byte gate and does not stop at zero.
    pub fn tick(
        &mut self,
        current: u32,
        player: u8,
        winner: u32,
        no_sale: bool,
        mut call: impl FnMut(TimerCall) -> Result<u32, String>,
    ) -> Result<(), String> {
        if current != u32::from(player) || winner != u32::MAX {
            call(TimerCall::Label { no_sale })?;
            return Ok(());
        }
        self.remaining = self.remaining.wrapping_sub(1);
        let seconds = (self.remaining as i32) / 50;
        if self.last != seconds {
            if self.sound != 0 {
                call(TimerCall::ReleaseSound(self.sound))?;
                self.sound = 0;
            }
            self.sound = call(TimerCall::CreateSound)?;
        }
        self.last = seconds.max(0);
        call(TimerCall::ClearText)?;
        call(TimerCall::WriteSeconds(self.last))?;
        Ok(())
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub buttons_enabled: u8,
    pub timer: TurnTimer,
    pub timer_visible: bool,
    pub timer_text: Option<TimerText>,
    pub turn_notice_count: u32,
    pub details: bool,
    pub ticks: u32,
    pub blink: bool,
    pub displayed_increment: u32,
    pub price_label: Option<u32>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bidder {
    pub current: u32,
    pub winner: u32,
    pub price: u32,
    pub passed: u8,
    pub withdrawn: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickCall {
    ClearText,
    Price(u32),
    /// The caller executes the real synchronous command, then returns its new
    /// controller projection. Eligibility must see the command's effects.
    Withdraw(u8),
    TimerVisible(bool),
    Tutorial(u32),
}
impl State {
    /// Original414730 message1; invoke at registered interface cadence after
    /// the separately visible timer panel, preserving native child order.
    pub fn tick(
        &mut self,
        mut bidder: Bidder,
        player: u8,
        tutorial_state: u8,
        mut call: impl FnMut(TickCall) -> Result<Option<Bidder>, String>,
    ) -> Result<(), String> {
        self.ticks = self.ticks.wrapping_add(1);
        self.blink = self.ticks & 63 > 31;
        call(TickCall::ClearText)?;
        self.price_label = None;
        if bidder.current == u32::from(player) {
            let price = bidder.price.wrapping_add(self.displayed_increment);
            call(TickCall::Price(price))?;
            self.price_label = Some(price);
        }
        if self.timer.remaining as i32 <= 0 && tutorial_state == 0 {
            self.timer.remaining = 800;
            bidder = call(TickCall::Withdraw(player))?
                .ok_or("Auction withdrawal did not return the authoritative controller")?;
        }
        let enabled = bidder.current == u32::from(player)
            && bidder.winner != u32::from(player)
            && bidder.passed == 0
            && bidder.withdrawn == 0;
        if enabled {
            self.buttons_enabled = 31;
            call(TickCall::TimerVisible(true))?;
            if !self.timer_visible {
                //43be20 invokes415b90 only on a newly shown timer panel.
                self.timer.sound = 0;
                self.timer.last = -1;
            }
            self.timer_visible = true;
            if self.turn_notice_count == 0 {
                call(TickCall::Tutorial(0x44))?;
                self.turn_notice_count = self.turn_notice_count.wrapping_add(1);
            }
        } else {
            call(TickCall::TimerVisible(false))?;
            self.timer_visible = false;
            self.buttons_enabled = 0;
            self.turn_notice_count = 0;
        }
        Ok(())
    }
    /// Message12 reaches this handler only after native button eligibility.
    /// The handler still checks the current participant before every action.
    pub fn button(
        &mut self,
        button: u32,
        current: u32,
        player: u8,
        price: u32,
        mut call: impl FnMut(Call) -> Result<bool, String>,
    ) -> Result<(), String> {
        if button == 5 {
            self.details = !self.details;
            return Ok(());
        }
        if button > 4 || current != u32::from(player) {
            return Ok(());
        }
        let command = if button < 3 {
            let amount = price.wrapping_add([50, 100, 500][button as usize]);
            // Original ADD wraps, then FILD signed DWORD/FSTP single precision.
            call(Call::Affordable(amount as i32 as f32))?.then_some(Command {
                action: 0,
                participant: u32::from(player),
                amount,
            })
        } else {
            Some(Command {
                action: if button == 3 { 2 } else { 1 },
                participant: u32::from(player),
                amount: 0,
            })
        };
        if let Some(command) = command {
            call(Call::Action(command))?;
            self.timer.remaining = 800;
            call(Call::HideTimer)?;
            self.timer_visible = false;
            self.buttons_enabled = 0;
            self.turn_notice_count = 0;
        }
        if button < 3 {
            // Original emits these tutorial events even when affordability fails.
            call(Call::Tutorial(0x41 + button))?;
        }
        Ok(())
    }
}

#[test]
#[ignore = "requires native auction button handler observations"]
fn original_auction_buttons() {
    let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_AUCTION_BUTTONS").unwrap()).unwrap();
    assert_eq!(&data[..8], b"MTAUBT01");
    let mut words = data[8..]
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    let count = words.next().unwrap();
    for case in 0..count {
        let input: [u32; 5] = std::array::from_fn(|_| words.next().unwrap());
        let expected: [u32; 5] = std::array::from_fn(|_| words.next().unwrap());
        let trace = (0..words.next().unwrap())
            .map(|_| std::array::from_fn::<_, 3, _>(|_| words.next().unwrap()))
            .collect::<Vec<_>>();
        let mut state = State {
            buttons_enabled: 31,
            timer: TurnTimer {
                remaining: 123,
                ..TurnTimer::default()
            },
            timer_visible: true,
            turn_notice_count: 7,
            details: true,
            ..State::default()
        };
        let mut actual = Vec::new();
        state
            .button(input[0], input[1], input[2] as u8, input[3], |call| {
                actual.push(match call {
                    Call::Affordable(amount) => [0, amount.to_bits(), 0],
                    Call::Action(c) => [
                        match c.action {
                            0 => 1,
                            2 => 2,
                            1 => 3,
                            _ => unreachable!(),
                        },
                        c.participant,
                        c.amount,
                    ],
                    Call::HideTimer => [4, 0, 0],
                    Call::Tutorial(code) => [5, code, 0],
                });
                Ok(input[4] != 0)
            })
            .unwrap();
        assert_eq!(actual, trace, "native call order case{case}");
        assert_eq!(
            [
                state.buttons_enabled as u32,
                state.timer.remaining,
                u32::from(state.timer_visible),
                state.turn_notice_count,
                u32::from(state.details)
            ],
            expected,
            "native state case{case}"
        );
    }
    assert!(words.next().is_none());
}

#[test]
#[ignore = "requires native auction turn timer observations"]
fn original_auction_turn_timer() {
    let data =
        std::fs::read(std::env::var("OPENMT2001_NATIVE_AUCTION_TURN_TIMER").unwrap()).unwrap();
    assert_eq!(&data[..8], b"MTAUTM01");
    let mut words = data[8..]
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    let count = words.next().unwrap();
    for case in 0..count {
        let input: [u32; 6] = std::array::from_fn(|_| words.next().unwrap());
        let expected: [u32; 3] = std::array::from_fn(|_| words.next().unwrap());
        let trace = (0..words.next().unwrap())
            .map(|_| std::array::from_fn::<_, 2, _>(|_| words.next().unwrap()))
            .collect::<Vec<_>>();
        let mut timer = TurnTimer {
            remaining: input[3],
            last: input[4] as i32,
            sound: input[5],
        };
        let mut actual = Vec::new();
        timer
            .tick(input[0], 2, input[1], input[2] != 0, |call| {
                actual.push(match call {
                    TimerCall::ReleaseSound(handle) => [0, handle],
                    TimerCall::CreateSound => [1, 0],
                    TimerCall::ClearText => [2, 0],
                    TimerCall::WriteSeconds(seconds) => [3, seconds as u32],
                    TimerCall::Label { no_sale } => [4, if no_sale { 0x533a74 } else { 0x533a5c }],
                });
                Ok(0x1357)
            })
            .unwrap();
        assert_eq!(
            [timer.remaining, timer.sound, timer.last as u32],
            expected,
            "native timer state case{case}"
        );
        assert_eq!(actual, trace, "native timer calls case{case}");
    }
    assert!(words.next().is_none());
}

#[test]
#[ignore = "requires native auction bidder eligibility observations"]
fn original_auction_bidder_tick() {
    let data =
        std::fs::read(std::env::var("OPENMT2001_NATIVE_AUCTION_BIDDER_TICK").unwrap()).unwrap();
    assert_eq!(&data[..8], b"MTAUBK01");
    let mut words = data[8..]
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    let count = words.next().unwrap();
    for case in 0..count {
        let input: [u32; 8] = std::array::from_fn(|_| words.next().unwrap());
        let expected: [u32; 6] = std::array::from_fn(|_| words.next().unwrap());
        let trace = (0..words.next().unwrap())
            .map(|_| std::array::from_fn::<_, 2, _>(|_| words.next().unwrap()))
            .collect::<Vec<_>>();
        let bidder = Bidder {
            current: input[0],
            winner: input[1],
            price: 1250,
            passed: input[2] as u8,
            withdrawn: input[3] as u8,
        };
        let mut state = State {
            buttons_enabled: 31,
            timer: TurnTimer {
                remaining: input[4],
                ..TurnTimer::default()
            },
            timer_visible: true,
            ticks: input[6],
            turn_notice_count: input[7],
            displayed_increment: 50,
            ..State::default()
        };
        let mut actual = Vec::new();
        state
            .tick(bidder, 2, input[5] as u8, |call| {
                actual.push(match call {
                    TickCall::ClearText => [0, 0],
                    TickCall::Price(value) => [1, value],
                    TickCall::Withdraw(player) => [2, u32::from(player)],
                    TickCall::TimerVisible(visible) => [3, u32::from(visible)],
                    TickCall::Tutorial(code) => [4, code],
                });
                Ok(matches!(call, TickCall::Withdraw(_)).then_some(Bidder {
                    current: 1,
                    passed: 1,
                    ..bidder
                }))
            })
            .unwrap();
        assert_eq!(
            [
                state.ticks,
                u32::from(state.blink),
                u32::from(state.buttons_enabled),
                state.timer.remaining,
                u32::from(state.timer_visible),
                state.turn_notice_count
            ],
            expected,
            "native bidder state case{case}"
        );
        assert_eq!(actual, trace, "native bidder call order case{case}");
    }
    assert!(words.next().is_none());
}
