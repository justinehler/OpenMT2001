//! Presentation fields written by original48bc70 through429xxx modal helpers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Argument {
    Name([u8; 96]),
    Language(u32),
    Integer(i32),
    Currency(i32),
}
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
                    Argument::Name(name.ok_or("Human dialog sender name unresolved")?),
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
    pub fn outcome(&mut self, call: openmt2001::outcome_notice::Call) -> Result<(), String> {
        use openmt2001::outcome_notice::{Argument as A, Call, Field};
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
                    A::Name(n) => Some(Argument::Name(n)),
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
                    let end = name
                        .iter()
                        .position(|b| *b == 0)
                        .ok_or("Human dialog actor name unterminated")?;
                    name[..end]
                        .iter()
                        .map(|b| {
                            b.checked_sub(32)
                                .filter(|b| *b <= 94)
                                .ok_or("Human dialog actor glyph mapping")
                        })
                        .collect::<Result<Vec<_>, _>>()?
                }
                Argument::Language(id) => strings
                    .get(*id as usize)
                    .ok_or("Human dialog substitution language missing")?
                    .clone(),
                Argument::Integer(value) => value.to_string().bytes().map(|b| b - 32).collect(),
                Argument::Currency(value) => crate::ui::integer_money(
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
    let bytes =
        std::fs::read(std::env::var("OPENMT2001_NATIVE_HUMAN_DIALOG_LAYOUT").unwrap()).unwrap();
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
