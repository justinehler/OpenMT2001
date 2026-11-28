//! Presentation fields written by original48bc70 through429xxx modal helpers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Argument {
    /// NUL-terminated code-page bytes.
    Name([u8; NAME_BYTES]),
    Language(u32),
    Integer(i32),
    Currency(i32),
}
/// A name argument's capacity: the 128-byte field cap (443aa0) and its
/// terminator. The original passes a pointer to the stored name (498f15,
/// 4981d0, 4950f0) and expands the whole body into a 0x400-byte buffer
/// (4e8410), so no name a field can hold is cut short.
pub const NAME_BYTES: usize = 129;
impl Argument {
    /// A name argument from code-page bytes, ending at the first NUL and
    /// keeping at most the 128 bytes a name field can hold.
    pub fn name(bytes: &[u8]) -> Self {
        let mut name = [0; NAME_BYTES];
        let text = bytes.split(|&b| b == 0).next().unwrap_or_default();
        let length = text.len().min(NAME_BYTES - 1);
        name[..length].copy_from_slice(&text[..length]);
        Self::Name(name)
    }
}
/// Portrait word for a modal with no portrait. Original activation 429b50
/// resets the portrait to 0xffffffff, and producers such as the in-city
/// (49e970) and desktop (495860) Quit prompts never call the 429af0 setter.
pub const NO_PORTRAIT: u32 = u32::MAX;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dialog {
    pub portrait: u32,
    pub title: u32,
    pub title_argument: Option<Argument>,
    pub timeout_ticks: Option<i32>,
    pub body: u32,
    pub arguments: [Argument; 3],
    pub argument_count: usize,
    pub buttons: [u32; 2],
    pub button_count: usize,
    pub callback: u32,
    pub context: u32,
    pub timeout: bool,
    pub handler_written: bool,
    pub context_written: bool,
    pub timeout_written: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reply {
    pub callback: u32,
    pub context: u32,
    pub button: u32,
}
impl Default for Dialog {
    fn default() -> Self {
        Self {
            portrait: 302,
            title: 0,
            title_argument: None,
            timeout_ticks: None,
            body: 0,
            arguments: [Argument::Integer(0); 3],
            argument_count: 0,
            buttons: [0; 2],
            button_count: 0,
            callback: 0,
            context: 0,
            timeout: true,
            handler_written: false,
            context_written: false,
            timeout_written: false,
        }
    }
}
impl Dialog {
    pub fn ui(
        &mut self,
        address: u32,
        arguments: [u32; 4],
        count: usize,
        name: Option<[u8; 96]>,
    ) -> Result<(), String> {
        match (address, count) {
            (0x429af0, 1) => self.portrait = arguments[0],
            (0x429820, 3) if arguments[0] <= 1 && arguments[2] == 0 => {
                if arguments[0] == 0 {
                    self.title = arguments[1];
                } else {
                    self.body = arguments[1];
                    self.argument_count = 0;
                }
            }
            (0x441940, 0) => {
                self.body = 0;
                self.argument_count = 0;
            }
            (0x441080, 2) => {
                self.body = arguments[0];
                self.arguments[0] = Argument::Language(arguments[1]);
                self.argument_count = 1;
            }
            (0x440d40, 4) if matches!(arguments[0], 0x6b1 | 0x6b3) => {
                self.body = arguments[0];
                self.arguments = [
                    Argument::name(&name.ok_or("Human dialog sender name unresolved")?),
                    if arguments[0] == 0x6b1 {
                        Argument::Integer(arguments[2] as i32)
                    } else {
                        Argument::Language(arguments[2])
                    },
                    Argument::Language(arguments[3]),
                ];
                self.argument_count = 3;
            }
            (0x429a60, 1) if (1..=2).contains(&arguments[0]) => {
                self.button_count = arguments[0] as usize
            }
            (0x4298b0, 2) if arguments[0] < 2 => self.buttons[arguments[0] as usize] = arguments[1],
            (0x429ac0, 1) if arguments[0] == 0 => {
                self.timeout = false;
                self.timeout_written = true;
            }
            _ => {
                return Err(format!(
                    "Human modal requires UI {address:08x} {arguments:?}/{count}"
                ));
            }
        }
        Ok(())
    }
    pub fn outcome(&mut self, call: crate::outcome_notice::Call) -> Result<(), String> {
        use crate::outcome_notice::{Argument as A, Call, Field};
        match call {
            Call::Timeout(seconds) => {
                self.timeout = seconds != 0;
                self.timeout_written = true;
                self.timeout_ticks = (seconds != 0).then_some(seconds.wrapping_mul(50) as i32);
            }
            Call::Clear(Field::Title) => {
                self.title = 0;
                self.title_argument = None;
            }
            Call::Clear(Field::Body) => {
                self.body = 0;
                self.argument_count = 0;
            }
            Call::Portrait(id) => self.portrait = id,
            Call::Buttons(n) if n <= 2 => self.button_count = n as usize,
            Call::Button { index, language } if index < 2 => {
                self.buttons[index as usize] = language
            }
            Call::Handler { callback, context } => {
                if let Some(c) = callback {
                    self.callback = c;
                    self.handler_written = true;
                }
                if let Some(c) = context {
                    self.context = c;
                    self.context_written = true;
                }
            }
            Call::Text {
                field,
                language,
                argument,
            } => {
                let value = match argument {
                    A::None => None,
                    A::Name(n) => Some(Argument::name(&n)),
                    A::Integer(n) => Some(Argument::Integer(n)),
                    A::Currency(n) => Some(Argument::Currency(n)),
                };
                match field {
                    Field::Title => {
                        self.title = language;
                        self.title_argument = value;
                    }
                    Field::Body => {
                        self.body = language;
                        self.argument_count = usize::from(value.is_some());
                        if let Some(v) = value {
                            self.arguments[0] = v;
                        }
                    }
                }
            }
            other => return Err(format!("Outcome modal requires {other:?}")),
        }
        Ok(())
    }
    pub fn title_text(&self, strings: &[Vec<u8>]) -> Result<Vec<u8>, String> {
        let mut title = *self;
        title.body = self.title;
        title.argument_count = usize::from(self.title_argument.is_some());
        if let Some(a) = self.title_argument {
            title.arguments[0] = a;
        }
        Ok(title.paragraphs(strings)?.remove(0))
    }
    pub fn paragraphs(&self, strings: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, String> {
        let mut text = strings
            .get(self.body as usize)
            .ok_or("Human dialog body language missing")?
            .clone();
        let mut from = 0;
        for argument in &self.arguments[..self.argument_count] {
            let at = text[from..]
                .windows(2)
                .position(|s| {
                    s[0] == b'%' - 32 && [b's' - 32, b'd' - 32, b'i' - 32].contains(&s[1])
                })
                .map(|at| at + from)
                .ok_or("Human dialog format placeholder missing")?;
            let value = match argument {
                Argument::Name(name) => {
                    // 4e7fe0/4e7690 draw each byte as row0[byte - 32] and end
                    // the substitution at the first byte below 32.
                    let row = strings.first().ok_or("Human dialog glyph row missing")?;
                    name.iter()
                        .take_while(|&&b| b >= 32)
                        .map(|&b| {
                            row.get(usize::from(b - 32))
                                .copied()
                                .ok_or("Human dialog name glyph missing")
                        })
                        .collect::<Result<Vec<_>, _>>()?
                }
                Argument::Language(id) => strings
                    .get(*id as usize)
                    .ok_or("Human dialog substitution language missing")?
                    .clone(),
                Argument::Integer(value) => value.to_string().bytes().map(|b| b - 32).collect(),
                Argument::Currency(value) => integer_money(
                    *value,
                    strings.get(2243).ok_or("Outcome currency symbol missing")?,
                    strings
                        .get(2244)
                        .ok_or("Outcome currency placement missing")?
                        .first()
                        .copied()
                        != Some(0x4c),
                ),
            };
            text.splice(at..at + 2, value.iter().copied());
            from = at + value.len();
        }
        Ok(vec![text])
    }
}

#[test]
fn name_arguments_draw_through_row_0_and_hold_a_full_field() {
    // A row that moves every glyph by one proves the name goes through it.
    let row: Vec<u8> = (0..224u8).map(|i| i.wrapping_add(1)).collect();
    let strings = vec![row.clone(), vec![b'%' - 32, b's' - 32]];
    let paragraph = |name: &[u8]| {
        let dialog = Dialog {
            body: 1,
            arguments: [
                Argument::name(name),
                Argument::Integer(0),
                Argument::Integer(0),
            ],
            argument_count: 1,
            ..Dialog::default()
        };
        dialog.paragraphs(&strings).unwrap().remove(0)
    };
    let name = [[0xe9; 127].as_slice(), b"Z"].concat();
    let expected: Vec<u8> = name.iter().map(|&b| row[usize::from(b - 32)]).collect();
    assert_eq!(paragraph(&name), expected);
    // A byte below 32 ends the substitution (4e7690), and a field never
    // holds more than 128 bytes.
    assert_eq!(paragraph(b"ab\ncd"), [row[65], row[66]]);
    assert_eq!(paragraph(&[b'x'; 200]).len(), 128);
}

/// Original429630: equal gaps around count60x24 buttons in the main region.
pub fn buttons(main: [f32; 4], count: usize) -> Vec<[f32; 4]> {
    let gap = (f64::from(main[2]) - f64::from(main[0]) - (count * 60) as f64) / (count + 1) as f64;
    let retained_gap = gap as f32;
    let mut x = (gap + f64::from(main[0])) as f32;
    let y = ((f64::from(main[3]) * 3. + f64::from(main[1])) * 0.25 + 2.) as f32;
    (0..count)
        .map(|_| {
            let right = f64::from(x) + 60.;
            let result = [x, y, right as f32, (f64::from(y) + 24.) as f32];
            x = (right + f64::from(retained_gap)) as f32;
            result
        })
        .collect()
}

#[test]
#[ignore = "requires native429630 button layout"]
fn original_modal_buttons() {
    let bytes = std::fs::read(crate::fixtures::path("NATIVE_HUMAN_DIALOG_LAYOUT")).unwrap();
    assert_eq!(&bytes[..8], b"MTHDLY01");
    let mut words = bytes[8..]
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    assert_eq!(words.next(), Some(0x37f));
    for case in 0..words.next().unwrap() {
        let count = words.next().unwrap() as usize;
        let main = std::array::from_fn(|_| f32::from_bits(words.next().unwrap()));
        for bounds in buttons(main, count) {
            let actual = [bounds[0], bounds[1], bounds[2], bounds[3], 0.5].map(f32::to_bits);
            for _ in 0..2 {
                let expected: [u32; 5] = std::array::from_fn(|_| words.next().unwrap());
                assert_eq!(actual, expected, "native modal{case}");
            }
        }
    }
    assert!(words.next().is_none());
}

/// Original43a8c0. The symbol and result use the game's glyph-byte encoding.
/// NEG wraps at signed MIN, including the original double-negative rendering.
pub fn integer_money(value: i32, symbol: &[u8], suffix: bool) -> Vec<u8> {
    let magnitude = if value < 0 {
        value.wrapping_neg()
    } else {
        value
    };
    let digits = magnitude
        .to_string()
        .bytes()
        .map(|b| b - 32)
        .collect::<Vec<_>>();
    let mut text = Vec::new();
    if value < 0 {
        text.push(b'-' - 32);
    }
    if !suffix {
        text.extend_from_slice(symbol);
    }
    text.extend_from_slice(&digits);
    if suffix {
        text.extend_from_slice(symbol);
    }
    text
}

/// The two Quit confirmations. They share their buttons and never set a
/// portrait, but differ in strings and callback.
pub mod quit {
    use super::{Dialog, NO_PORTRAIT};

    /// Callback of the in-city abandonment prompt (original 49e940).
    pub const CITY_CALLBACK: u32 = 0x49e940;
    /// Callback of the main-menu Quit-to-desktop prompt (original 495670).
    pub const DESKTOP_CALLBACK: u32 = 0x495670;

    /// Original 49e970's abandonment prompt (strings 0x6e3/0x6e4).
    pub fn city() -> Dialog {
        Dialog {
            portrait: NO_PORTRAIT,
            title: 1763,
            body: 1764,
            buttons: [1636, 1637],
            button_count: 2,
            callback: CITY_CALLBACK,
            timeout: false,
            handler_written: true,
            context_written: true,
            timeout_written: true,
            ..Default::default()
        }
    }

    /// Original 4958f4's main-menu Quit confirmation.
    pub fn desktop() -> Dialog {
        Dialog {
            portrait: NO_PORTRAIT,
            title: 1765,
            body: 1766,
            buttons: [1636, 1637],
            button_count: 2,
            callback: DESKTOP_CALLBACK,
            context: 0,
            timeout: false,
            handler_written: true,
            context_written: true,
            timeout_written: true,
            ..Dialog::default()
        }
    }
}
