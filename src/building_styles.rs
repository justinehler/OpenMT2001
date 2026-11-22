/// Ordered original437320 TexturePrestigeRanking options from an era parameter file.
#[derive(Clone, Debug, Default)]
pub struct Styles {
    pub qualities: [Vec<u32>; 3],
}
impl Styles {
    pub fn from_parameters(source: &str) -> Result<Self, String> {
        if !source.is_ascii() {
            return Err("building style parameters must be original ASCII".into());
        }
        let mut result = Self::default();
        for line in source.lines() {
            let mut words = line.split_ascii_whitespace();
            if !words
                .next()
                .is_some_and(|s| s.eq_ignore_ascii_case("TexturePrestigeRanking"))
            {
                continue;
            }
            let token = words
                .next()
                .ok_or("missing building style token")?
                .to_ascii_uppercase();
            if token == "RESET" {
                result.qualities.iter_mut().for_each(Vec::clear);
                continue;
            }
            let quality = match words.next().map(str::to_ascii_uppercase).as_deref() {
                Some("LOW") => 0,
                Some("MEDIUM") => 1,
                Some("HIGH") => 2,
                Some("NA") => continue,
                _ => return Err("unknown building style quality".into()),
            };
            let (suffix, base) = [
                ("STONE", 0u32),
                ("BRICK", 0x50),
                ("CONCRETE", 0xf0),
                ("TIMBER", 0x190),
                ("GLASS", 0x230),
            ]
            .into_iter()
            .find_map(|(name, base)| token.strip_prefix(name).map(|suffix| (suffix, base)))
            .ok_or("unknown building style family")?;
            let number = suffix
                .parse::<i32>()
                .map_err(|_| "invalid building style number")?;
            result.qualities[quality].push(
                (number.wrapping_sub(1) as u32)
                    .wrapping_shl(4)
                    .wrapping_add(base),
            );
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "private original eight-era style registration oracle"]
    fn original_era_styles() {
        let b = std::fs::read(std::env::var("OPENMT2001_NATIVE_BUILD_STYLES").unwrap()).unwrap();
        assert_eq!(&b[..8], b"MTBLSTYL");
        let word = |at: &mut usize| {
            let v = u32::from_le_bytes(b[*at..*at + 4].try_into().unwrap());
            *at += 4;
            v
        };
        let mut at = 8;
        assert_eq!(word(&mut at), 8);
        for year in (1930..=2000).step_by(10) {
            assert_eq!(word(&mut at), year);
            let n = word(&mut at) as usize;
            let styles =
                Styles::from_parameters(std::str::from_utf8(&b[at..at + n]).unwrap()).unwrap();
            at += n;
            for quality in styles.qualities {
                let n = word(&mut at);
                let expected: Vec<_> = (0..n).map(|_| word(&mut at)).collect();
                assert_eq!(quality, expected, "{year}");
            }
        }
        assert_eq!(at, b.len());
    }
}
