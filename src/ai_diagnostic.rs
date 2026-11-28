//! Typed calls to the original 4e6960 diagnostic dispatcher from the AI and
//! the player-message broadcaster (0x48c1c0).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Kind {
    Fatal = 0,
    RecoverableWarning = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Message {
    Original(u32),
    Timetable { secondary: bool, code: i32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub kind: Kind,
    pub message: Message,
}

impl Diagnostic {
    pub fn original(kind: u32, text: u32) -> Result<Self, String> {
        Ok(Self {
            kind: match kind {
                0 => Kind::Fatal,
                1 => Kind::RecoverableWarning,
                _ => return Err(format!("unsupported AI diagnostic kind {kind}")),
            },
            message: Message::Original(text),
        })
    }

    pub fn warning(text: u32) -> Self {
        Self {
            kind: Kind::RecoverableWarning,
            message: Message::Original(text),
        }
    }

    pub fn timetable(secondary: bool, code: i32) -> Self {
        Self {
            kind: Kind::RecoverableWarning,
            message: Message::Timetable { secondary, code },
        }
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.message {
            Message::Original(text) => {
                let description = match text {
                    0x53da30 => "offer completion has no matching offer",
                    0x53d9fc => "offer completion received in an unexpected state",
                    0x53d9e0 => "unsupported offer predecision type",
                    0x541d70 => "unhandled city message",
                    _ => "original AI diagnostic",
                };
                write!(f, "{description} (message {text:08x})")
            }
            Message::Timetable { secondary, code } => write!(
                f,
                "unsupported {} timetable event code {code}",
                if secondary { "secondary" } else { "primary" },
            ),
        }
    }
}
