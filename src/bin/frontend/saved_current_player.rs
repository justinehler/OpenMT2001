//! Original normal local-load identity, not a serialized current-player field.
//!4bcbf6->4b9d40->48ce00 clears86cfc0; ordered tag40 restore48c800
//!selects the first restored player only while that pointer is null.
use openmt2001::save::SaveFile;
pub struct Player {
    pub slot: usize,
    pub bytes: [u8; 440],
}
pub fn normal_load(save: &SaveFile<'_>) -> Result<Option<Player>, String> {
    let players = save.players()?;
    if players.len() > 6 {
        return Err("normal saved load exceeds six player slots".into());
    }
    for p in &players {
        let kind = u32::from_le_bytes(p.bytes[0x150..0x154].try_into().unwrap());
        if kind > 2 {
            return Err("unsupported saved player constructor".into());
        }
    }
    Ok(players.first().map(|p| Player {
        slot: 0,
        bytes: *p.bytes,
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "private normal-load player oracle"]
    fn matches_native_saved_current_player() {
        let path = openmt2001::fixtures::path("NATIVE_SAVED_CURRENT_PLAYER");
        let b = std::fs::read(&path).unwrap();
        assert_eq!(&b[..8], b"MTCURPL1");
        let mut at = 8;
        let word = |at: &mut usize| {
            let n = u32::from_le_bytes(b[*at..*at + 4].try_into().unwrap());
            *at += 4;
            n
        };
        let count = word(&mut at);
        for _ in 0..count {
            let n = word(&mut at) as usize;
            let name = std::str::from_utf8(&b[at..at + n]).unwrap();
            at += n + 32;
            let _occupied = word(&mut at);
            let n = word(&mut at);
            let file = ["OPENMT2001_SAVE_ONE", "OPENMT2001_SAVE_TWO"]
                .into_iter()
                .map(|k| openmt2001::fixtures::path(k))
                .find(|p| p.file_name().unwrap() == name)
                .unwrap_or_else(|| path.parent().unwrap().join(name));
            let bytes = std::fs::read(file).unwrap();
            let save = SaveFile::parse(&bytes).unwrap();
            let selected = normal_load(&save).unwrap().unwrap();
            assert_eq!(save.players().unwrap().len(), n as usize);
            for created in 0..n {
                assert_eq!(word(&mut at), created);
                assert_eq!(word(&mut at) as usize, selected.slot);
                at += 32;
                assert_eq!(&b[at..at + 440], selected.bytes);
                at += 440;
            }
        }
        assert_eq!(at, b.len());
    }
}
