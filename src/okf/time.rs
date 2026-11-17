//! The ISO 8601 forms OKF uses (§5, §9), without a date-time dependency.

fn digits(text: &str) -> Option<i64> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days since 1970-01-01 for a proleptic Gregorian date.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Parses `YYYY-MM-DD` and returns days since the Unix epoch.
fn parse_date(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let year = digits(&text[0..4])?;
    let month = digits(&text[5..7])?;
    let day = digits(&text[8..10])?;
    if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
        return None;
    }
    Some(days_from_civil(year, month, day))
}

/// Whether `text` is an ISO 8601 calendar date in `YYYY-MM-DD` form.
pub fn is_date(text: &str) -> bool {
    parse_date(text).is_some()
}

/// Parses an ISO 8601 datetime with an explicit UTC offset, such as
/// `2026-06-30T14:00:00Z` or `2026-06-30T14:00:00.5+02:00`, into Unix
/// seconds. Datetimes without an offset are refused: they name a different
/// instant in every time zone.
pub fn parse_datetime(text: &str) -> Option<i64> {
    if !text.is_ascii() || text.len() < 20 {
        return None;
    }
    let days = parse_date(&text[..10])?;
    if !matches!(text.as_bytes()[10], b'T' | b't') {
        return None;
    }
    let rest = &text[11..];
    if rest.len() < 9 || &rest[2..3] != ":" || &rest[5..6] != ":" {
        return None;
    }
    let hour = digits(&rest[0..2])?;
    let minute = digits(&rest[3..5])?;
    let second = digits(&rest[6..8])?;
    // 60 allows a leap second.
    if hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let mut zone = &rest[8..];
    if let Some(fraction) = zone.strip_prefix(['.', ',']) {
        let len = fraction.bytes().take_while(u8::is_ascii_digit).count();
        if len == 0 {
            return None;
        }
        zone = &fraction[len..];
    }
    let offset = match zone {
        "Z" | "z" => 0,
        _ => {
            let sign = match zone.as_bytes().first() {
                Some(b'+') => 1,
                Some(b'-') => -1,
                _ => return None,
            };
            let body = &zone[1..];
            let (h, m) = match body.len() {
                5 if &body[2..3] == ":" => (digits(&body[0..2])?, digits(&body[3..5])?),
                4 => (digits(&body[0..2])?, digits(&body[2..4])?),
                2 => (digits(body)?, 0),
                _ => return None,
            };
            if h > 23 || m > 59 {
                return None;
            }
            sign * (h * 3600 + m * 60)
        }
    };
    Some(days * 86_400 + hour * 3600 + minute * 60 + second - offset)
}

/// Formats Unix seconds as `YYYY-MM-DDTHH:MM:SSZ`.
pub fn format_utc(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let secs = seconds.rem_euclid(86_400);
    // Inverse of days_from_civil (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        secs / 3600,
        secs % 3600 / 60,
        secs % 60
    )
}

/// The current Unix time in seconds.
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_datetimes_with_offsets() {
        assert_eq!(parse_datetime("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_datetime("1970-01-01T01:00:00+01:00"), Some(0));
        assert_eq!(parse_datetime("1970-01-01T00:00:00.250-0030"), Some(1800));
        assert_eq!(parse_datetime("2026-06-30T14:00:00Z"), Some(1_782_828_000));
    }

    #[test]
    fn refuses_datetimes_without_offsets_or_with_bad_fields() {
        for text in [
            "2026-06-30",
            "2026-06-30T14:00:00",
            "2026-06-30 14:00:00Z",
            "2026-02-30T14:00:00Z",
            "2026-06-30T24:00:00Z",
            "2026-06-30T14:00:00+2:00",
            "2026-06-30T14:00:00.Z",
        ] {
            assert_eq!(parse_datetime(text), None, "{text}");
        }
    }

    #[test]
    fn checks_dates() {
        assert!(is_date("2024-02-29"));
        assert!(!is_date("2026-02-29"));
        assert!(!is_date("2026-5-22"));
        assert!(!is_date("Changes"));
    }

    #[test]
    fn formats_utc() {
        assert_eq!(format_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_utc(1_782_828_000), "2026-06-30T14:00:00Z");
        assert_eq!(parse_datetime(&format_utc(951_782_400)), Some(951_782_400));
    }
}
