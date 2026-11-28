//! The save lists' modified-time label (497708). Native only: it needs chrono.
use std::time::SystemTime;

/// The label's morning and afternoon suffixes: language rows 1374 and 1375,
/// the same glyph rows the HUD clock uses.
#[derive(Clone, Copy)]
pub struct DaySuffixes<'a> {
    pub am: &'a [u8],
    pub pm: &'a [u8],
}

impl<'a> DaySuffixes<'a> {
    /// Rows 1374/1375 of a loaded language table; missing rows stay empty.
    pub fn from_strings<S: AsRef<[u8]>>(strings: &'a [S]) -> Self {
        let row = |id: usize| strings.get(id).map_or(&[][..], AsRef::as_ref);
        Self {
            am: row(1374),
            pm: row(1375),
        }
    }
}

/// Original 497708 uses CRT localtime for the displayed hour/minute, then
/// FileTimeToSystemTime for the date, so the date remains UTC near a local
/// day boundary. GetDateFormat uses the user's short-date format. The result
/// is glyph text (ASCII minus 32, like the language rows it embeds).
pub fn format_modified_local(
    modified: Option<SystemTime>,
    suffixes: DaySuffixes<'_>,
) -> Option<Vec<u8>> {
    let utc: chrono::DateTime<chrono::Utc> = modified?.into();
    Some(original_modified_label(
        utc.with_timezone(&chrono::Local),
        user_short_date,
        suffixes,
    ))
}

fn original_modified_label<Tz: chrono::TimeZone>(
    local: chrono::DateTime<Tz>,
    format_date: impl FnOnce(chrono::NaiveDate) -> String,
    suffixes: DaySuffixes<'_>,
) -> Vec<u8> {
    use chrono::Timelike;
    original_time_label(
        local.hour(),
        local.minute(),
        &format_date(local.naive_utc().date()),
        suffixes,
    )
}

#[cfg(windows)]
fn user_short_date(date: chrono::NaiveDate) -> String {
    windows_short_date(date).unwrap_or_else(|| short_date_for_locale(date, "C"))
}

#[cfg(windows)]
fn windows_short_date(date: chrono::NaiveDate) -> Option<String> {
    use chrono::Datelike;
    use windows_sys::Win32::{
        Foundation::SYSTEMTIME,
        Globalization::{GetDateFormatW, LOCALE_USER_DEFAULT},
    };
    let system_time = SYSTEMTIME {
        wYear: date.year().try_into().ok()?,
        wMonth: date.month() as u16,
        wDay: date.day() as u16,
        ..Default::default()
    };
    // SAFETY: SYSTEMTIME is initialized and lives through both calls. A null
    // format selects the user's short date, including any system overrides.
    // With a null output and zero capacity, this call only queries its size.
    let size = unsafe {
        GetDateFormatW(
            LOCALE_USER_DEFAULT,
            0,
            &system_time,
            std::ptr::null(),
            std::ptr::null_mut(),
            0,
        )
    };
    if size <= 0 {
        return None;
    }
    let mut buffer = vec![0u16; size as usize];
    // SAFETY: The writable buffer has the requested number of UTF-16 units.
    let written = unsafe {
        GetDateFormatW(
            LOCALE_USER_DEFAULT,
            0,
            &system_time,
            std::ptr::null(),
            buffer.as_mut_ptr(),
            size,
        )
    };
    (written > 0).then(|| String::from_utf16_lossy(&buffer[..written as usize - 1]))
}

#[cfg(not(windows))]
fn user_short_date(date: chrono::NaiveDate) -> String {
    let locale = date_locale(|key| std::env::var(key).ok());
    short_date_for_locale(date, locale.as_deref().unwrap_or("C"))
}

#[cfg(any(not(windows), test))]
fn date_locale(mut lookup: impl FnMut(&str) -> Option<String>) -> Option<String> {
    ["LC_ALL", "LC_TIME", "LANG"]
        .into_iter()
        .filter_map(&mut lookup)
        .find(|locale| !locale.is_empty())
}

/// A small fallback for hosts without Windows' user-date API. Recognize common
/// locale orders/separators; C, POSIX and unknown locales keep the ISO fallback.
fn short_date_for_locale(date: chrono::NaiveDate, locale: &str) -> String {
    let locale = locale
        .split(['.', '@'])
        .next()
        .unwrap_or(locale)
        .replace('-', "_")
        .to_ascii_lowercase();
    let language = locale.split('_').next().unwrap_or(&locale);
    let pattern = match locale.as_str() {
        "en_gb" | "en_au" | "en_nz" | "en_ie" | "en_in" | "en_sg" => "%d/%m/%Y",
        "en_ca" | "fr_ca" => "%Y-%m-%d",
        "de_ch" | "fr_ch" | "it_ch" => "%d.%m.%Y",
        _ => match language {
            "en" => "%-m/%-d/%Y",
            "fr" | "es" | "it" | "pt" => "%d/%m/%Y",
            "de" | "ru" | "pl" => "%d.%m.%Y",
            "fi" => "%-d.%-m.%Y",
            "nl" => "%d-%m-%Y",
            "ja" | "zh" => "%Y/%m/%d",
            _ => "%Y-%m-%d",
        },
    };
    date.format(pattern).to_string()
}

fn original_time_label(hour: u32, minute: u32, date: &str, suffixes: DaySuffixes<'_>) -> Vec<u8> {
    // 49772c subtracts 12 only when greater; 497783 changes zero to 12.
    // The original still gives noon the morning suffix.
    let (hour, suffix) = if hour > 12 {
        (hour - 12, suffixes.pm)
    } else {
        (hour, suffixes.am)
    };
    let hour = if hour == 0 { 12 } else { hour };
    let mut label = crate::level_text::ascii(&format!("{hour}:{minute:02}"));
    label.extend_from_slice(suffix);
    label.extend(crate::level_text::ascii(&format!(" {date}")));
    label
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Glyph rows as a language table stores them (ASCII minus 32).
    fn glyphs(text: &str) -> Vec<u8> {
        crate::level_text::ascii(text)
    }

    fn time_label(hour: u32, minute: u32, date: &str) -> Vec<u8> {
        let (am, pm) = (glyphs("am"), glyphs("pm"));
        original_time_label(hour, minute, date, DaySuffixes { am: &am, pm: &pm })
    }

    #[test]
    fn save_list_clock_suffixes_come_from_language_rows_1374_and_1375() {
        let mut strings = vec![Vec::new(); 1376];
        strings[1374] = glyphs("vorm.");
        strings[1375] = glyphs("nachm.");
        let suffixes = DaySuffixes::from_strings(&strings);
        assert_eq!(
            original_time_label(9, 5, "21.06.2026", suffixes),
            glyphs("9:05vorm. 21.06.2026")
        );
        assert_eq!(
            original_time_label(17, 21, "21.06.2026", suffixes),
            glyphs("5:21nachm. 21.06.2026")
        );
        let missing = DaySuffixes::from_strings::<Vec<u8>>(&[]);
        assert_eq!(original_time_label(17, 21, "x", missing), glyphs("5:21 x"));
    }

    #[test]
    fn original_save_list_clock_changes_zero_to_twelve_but_keeps_noon_suffix() {
        assert_eq!(time_label(0, 5, "2026-06-21"), glyphs("12:05am 2026-06-21"));
        assert_eq!(
            time_label(12, 5, "2026-06-21"),
            glyphs("12:05am 2026-06-21")
        );
        assert_eq!(
            time_label(17, 21, "2026-06-21"),
            glyphs("5:21pm 2026-06-21")
        );
        assert_eq!(
            time_label(23, 59, "2026-06-21"),
            glyphs("11:59pm 2026-06-21")
        );
    }

    #[test]
    fn short_dates_follow_locale_order_separator_and_padding() {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 6, 9).unwrap();
        for (locale, expected) in [
            ("en_US.UTF-8", "6/9/2026"),
            ("en-GB", "09/06/2026"),
            ("de_DE.utf8@euro", "09.06.2026"),
            ("fr_FR", "09/06/2026"),
            ("fr-CH", "09.06.2026"),
            ("nl_NL", "09-06-2026"),
            ("ja_JP", "2026/06/09"),
            ("fr_CA", "2026-06-09"),
            ("C", "2026-06-09"),
            ("C.UTF-8", "2026-06-09"),
            ("POSIX", "2026-06-09"),
            ("unknown", "2026-06-09"),
        ] {
            assert_eq!(short_date_for_locale(date, locale), expected, "{locale}");
        }
    }

    #[test]
    fn date_locale_obeys_environment_precedence_and_ignores_empty_values() {
        let resolve = |values: [Option<&str>; 3]| {
            date_locale(|key| {
                let index = ["LC_ALL", "LC_TIME", "LANG"]
                    .iter()
                    .position(|candidate| *candidate == key)
                    .unwrap();
                values[index].map(str::to_owned)
            })
        };
        assert_eq!(
            resolve([Some("C"), Some("de_DE"), Some("en_US")]),
            Some("C".into())
        );
        assert_eq!(
            resolve([Some(""), Some("de_DE"), Some("en_US")]),
            Some("de_DE".into())
        );
        assert_eq!(
            resolve([None, Some(""), Some("en_US")]),
            Some("en_US".into())
        );
        assert_eq!(resolve([None, None, None]), None);
    }

    #[test]
    fn save_timestamp_keeps_utc_date_when_local_time_is_on_the_previous_day() {
        let utc = chrono::DateTime::parse_from_rfc3339("2026-06-22T01:21:00Z").unwrap();
        let local = utc.with_timezone(&chrono::FixedOffset::west_opt(4 * 3600).unwrap());
        let (am, pm) = (glyphs("am"), glyphs("pm"));
        assert_eq!(
            original_modified_label(
                local,
                |date| short_date_for_locale(date, "en_US"),
                DaySuffixes { am: &am, pm: &pm }
            ),
            glyphs("9:21pm 6/22/2026")
        );
    }

    #[test]
    fn save_timestamp_keeps_utc_date_when_local_midnight_is_on_the_next_day() {
        let utc = chrono::DateTime::parse_from_rfc3339("2026-06-21T22:05:00Z").unwrap();
        let local = utc.with_timezone(&chrono::FixedOffset::east_opt(2 * 3600).unwrap());
        let (am, pm) = (glyphs("am"), glyphs("pm"));
        assert_eq!(
            original_modified_label(
                local,
                |date| short_date_for_locale(date, "de_DE"),
                DaySuffixes { am: &am, pm: &pm }
            ),
            glyphs("12:05am 21.06.2026")
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_short_date_uses_the_system_formatter() {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 6, 9).unwrap();
        assert!(windows_short_date(date).is_some_and(|label| !label.is_empty()));
    }
}
