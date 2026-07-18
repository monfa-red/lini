//! The `format:` engine's date half [SPEC 14.3/14.4/16]: ISO-8601 literals to
//! epoch seconds, the civil-calendar math both directions, and date **text**
//! (the presets' renderings). All math is UTC — a bare date is midnight UTC,
//! an offset keeps its instant, and rendering never reads a clock or a
//! timezone, so output is byte-identical everywhere. Calendar tick *selection*
//! is the chart's job ([`crate::layout::chart`]); this file is the calendar.

use super::format::DateUnit;

/// Days from 1970-01-01 for a civil date (proleptic Gregorian) — Howard
/// Hinnant's `days_from_civil`, exact over the whole `i64` day range.
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = (m as i64 + 9) % 12; // Mar=0 … Feb=11
    let doy = (153 * mp + 2) / 5 + d as i64 - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146097 + doe - 719468
}

/// The inverse: a civil `(year, month, day)` from days since 1970-01-01.
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn days_in_month(y: i64, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
                29
            } else {
                28
            }
        }
    }
}

/// Parse an ISO-8601 literal to **epoch seconds** [SPEC 14.3]: `YYYY-MM-DD`,
/// optionally `THH:MM[:SS]`, optionally `Z` / `±HH:MM`. A bare date is
/// midnight UTC; an offset keeps its instant. `None` = not a date.
pub fn parse(s: &str) -> Option<f64> {
    let b = s.as_bytes();
    let digits = |r: std::ops::Range<usize>| -> Option<i64> {
        if r.end > b.len() || !b[r.clone()].iter().all(u8::is_ascii_digit) {
            return None;
        }
        s[r].parse().ok()
    };
    if b.len() < 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let (y, m, d) = (digits(0..4)?, digits(5..7)? as u32, digits(8..10)? as u32);
    if !(1..=12).contains(&m) || d < 1 || d > days_in_month(y, m) {
        return None;
    }
    let mut secs = days_from_civil(y, m, d) as f64 * 86_400.0;
    let mut i = 10;
    if i < b.len() && b[i] == b'T' {
        if b.len() < i + 6 || b[i + 3] != b':' {
            return None;
        }
        let (hh, mm) = (digits(i + 1..i + 3)?, digits(i + 4..i + 6)?);
        i += 6;
        let ss = if i < b.len() && b[i] == b':' {
            if b.len() < i + 3 {
                return None;
            }
            let v = digits(i + 1..i + 3)?;
            i += 3;
            v
        } else {
            0
        };
        if hh >= 24 || mm >= 60 || ss >= 60 {
            return None;
        }
        secs += (hh * 3600 + mm * 60 + ss) as f64;
    }
    match b.get(i) {
        None => {}
        Some(b'Z') if i + 1 == b.len() => {}
        Some(sign @ (b'+' | b'-')) => {
            if b.len() != i + 6 || b[i + 3] != b':' {
                return None;
            }
            let (oh, om) = (digits(i + 1..i + 3)?, digits(i + 4..i + 6)?);
            if oh >= 24 || om >= 60 {
                return None;
            }
            let off = (oh * 3600 + om * 60) as f64;
            // The literal is local-to-its-offset; normalize to the instant.
            secs += if *sign == b'+' { -off } else { off };
        }
        Some(_) => return None,
    }
    Some(secs)
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// A tick instant's text at a preset's granularity [SPEC 14.4/16]: `2026`,
/// `Jan 2026`, `Mar 4`, `09:00`, `09:30`.
pub fn render(epoch: f64, unit: DateUnit) -> String {
    let secs = epoch.floor() as i64;
    let days = secs.div_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let tod = secs.rem_euclid(86_400);
    match unit {
        DateUnit::Year => format!("{y}"),
        DateUnit::Month => format!("{} {y}", MONTHS[m as usize - 1]),
        DateUnit::Day => format!("{} {d}", MONTHS[m as usize - 1]),
        DateUnit::Hour | DateUnit::Minute => {
            format!("{:02}:{:02}", tod / 3600, (tod % 3600) / 60)
        }
    }
}

/// A datum's full instant for hover text [SPEC 14.8]: `Mar 4 2026`, plus
/// `, 09:30` when it carries a time of day.
pub fn render_full(epoch: f64) -> String {
    let secs = epoch.floor() as i64;
    let days = secs.div_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let tod = secs.rem_euclid(86_400);
    if tod == 0 {
        format!("{} {d} {y}", MONTHS[m as usize - 1])
    } else {
        format!(
            "{} {d} {y}, {:02}:{:02}",
            MONTHS[m as usize - 1],
            tod / 3600,
            (tod % 3600) / 60
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_math_round_trips_known_dates() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11017);
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        for z in [-1_000_000, -1, 0, 59, 365, 730_500, 1_000_000] {
            let (y, m, d) = civil_from_days(z);
            assert_eq!(days_from_civil(y, m, d), z);
        }
        // Leap rules: 2000 was a leap year, 1900 was not.
        assert_eq!(days_in_month(2000, 2), 29);
        assert_eq!(days_in_month(1900, 2), 28);
        assert_eq!(days_in_month(2024, 2), 29);
    }

    #[test]
    fn parses_the_iso_forms() {
        assert_eq!(parse("1970-01-01"), Some(0.0));
        assert_eq!(parse("1970-01-02"), Some(86_400.0));
        assert_eq!(parse("1970-01-01T01:30"), Some(5_400.0));
        assert_eq!(parse("1970-01-01T00:00:30Z"), Some(30.0));
        // An offset keeps its instant: 02:00+02:00 is midnight UTC.
        assert_eq!(parse("1970-01-01T02:00+02:00"), Some(0.0));
        assert_eq!(parse("1969-12-31T23:00-01:00"), Some(0.0));
        assert_eq!(
            parse("2026-01-01"),
            Some(days_from_civil(2026, 1, 1) as f64 * 86_400.0)
        );
    }

    #[test]
    fn rejects_non_dates() {
        for bad in [
            "2026-13-01",
            "2026-02-30",
            "1900-02-29",
            "2026-1-01",
            "2026-01-01T25:00",
            "2026-01-01T09",
            "2026-01-01X",
            "2026-01-01T09:00+2:00",
            "hello",
            "20260101",
        ] {
            assert_eq!(parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn renders_the_full_instant() {
        assert_eq!(render_full(parse("2026-03-04").unwrap()), "Mar 4 2026");
        assert_eq!(
            render_full(parse("2026-03-04T09:30").unwrap()),
            "Mar 4 2026, 09:30"
        );
    }

    #[test]
    fn renders_each_preset() {
        let t = parse("2026-03-04T09:30").unwrap();
        assert_eq!(render(t, DateUnit::Year), "2026");
        assert_eq!(render(t, DateUnit::Month), "Mar 2026");
        assert_eq!(render(t, DateUnit::Day), "Mar 4");
        assert_eq!(render(t, DateUnit::Hour), "09:30");
        assert_eq!(render(t, DateUnit::Minute), "09:30");
    }
}
