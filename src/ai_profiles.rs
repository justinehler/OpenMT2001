//! Original character-specific switches used by valuation and candidate choice.
use crate::pe::PeImage;

///47ce10. Signed additive amount, including the original zero default.
pub fn adjustment(character: u32) -> i32 {
    [-650, 850, 450, -1000, 900, -400, 600, 250, 1000, -700]
        .get(character as usize)
        .copied()
        .unwrap_or(0)
}
///47d110. Original character preference used by candidate and auction scoring.
pub fn preferred_tier(character: u32) -> i32 {
    [2, 3, 3, 5, 1, 3, 4, 2, 1, 4]
        .get(character as usize)
        .copied()
        .unwrap_or(0)
}
///47d170. Multiplier for the retained loan term query.
pub fn loan_multiplier(character: u32) -> i32 {
    [30, 50, 70, 90, 100, 50, 30, 20, 70, 100]
        .get(character as usize)
        .copied()
        .unwrap_or(0)
}
///47d250. Signed additive amount in the original budget calculations.
pub fn reserve(character: u32) -> i32 {
    [1650, 150, 550, 2000, 100, 1400, 400, 750, 0, 1700]
        .get(character as usize)
        .copied()
        .unwrap_or(0)
}

///The original switch returns a PE f32 constant without arithmetic. Unknown
///characters select original525040, while unknown routines are unsupported.
pub fn factor(image: &PeImage<'_>, address: u32, character: u32) -> Result<f32, String> {
    let rows = match address {
        0x47d360 => [0x525d20,0x5251dc,0x52505c,0x525d18,0x525d14,0x525d1c,0x52505c,0x525d10,0x525d14,0x5251dc],
        0x47ce90 => [
            0x525cf4, 0x525cf0, 0x525b10, 0x525034, 0x525cf0, 0x525cf4, 0x525cf0, 0x525b10,
            0x525cf0, 0x525cf4,
        ],
        0x47cf00 => [
            0x525b64, 0x525bd0, 0x5251e8, 0x525034, 0x5251e8, 0x525cf4, 0x5251e8, 0x525c78,
            0x525cf0, 0x525b10,
        ],
        0x47cf90 => [
            0x525cf0, 0x525b10, 0x525bd0, 0x525034, 0x5251e8, 0x525c78, 0x525b64, 0x525bd0,
            0x525b64, 0x5251e8,
        ],
        0x47d010 => [
            0x525b64, 0x525af4, 0x525c50, 0x525034, 0x525cf8, 0x525c50, 0x525c50, 0x52502c,
            0x525af4, 0x525cdc,
        ],
        0x47d090 => [
            0x525cdc, 0x525bd0, 0x52502c, 0x525bd8, 0x525af4, 0x52502c, 0x525cdc, 0x525b64,
            0x525af4, 0x525c50,
        ],
        0x47d1d0 => [
            0x525b64, 0x525c78, 0x525b10, 0x525034, 0x525034, 0x525cf4, 0x5251e8, 0x525c78,
            0x525cf4, 0x525bd0,
        ],
        _ => return Err(format!("AI profile factor {address:08x} is unbound")),
    };
    let at = rows.get(character as usize).copied().unwrap_or(0x525040);
    Ok(f32::from_le_bytes(image.at(at, 4)?.try_into().unwrap()))
}
#[test]
#[ignore = "requires original AI profile capture and executable"]
fn matches_native_profiles() {
    use crate::player_manager::word;
    let d = std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_PROFILES").unwrap()).unwrap();
    let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
    let image = PeImage::parse(&exe).unwrap();
    assert_eq!(&d[..8], b"MTAIPR01");
    assert_eq!(word(&d, 8), 0x37f);
    assert_eq!(d.len(), 16 + word(&d, 12) as usize * 12);
    for c in d[16..].chunks_exact(12) {
        let a = word(c, 0);
        let character = word(c, 4);
        let actual = match a {
            0x47ce10 => adjustment(character) as u32,
            0x47d110 => preferred_tier(character) as u32,
            0x47d170 => loan_multiplier(character) as u32,
            0x47d250 => reserve(character) as u32,
            _ => factor(&image, a, character).unwrap().to_bits(),
        };
        assert_eq!(actual, word(c, 8), "routine {a:08x}, character {character}");
    }
}
