//! Dates as the history list shows them: relative while recent, absolute after.

use jiff::civil::Weekday;
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan};

/// `now`, `12m`, `5h` today; `Yesterday`; a weekday earlier this week (weeks start on Monday);
/// `Nov 8` earlier this year; `Nov 8, 2025` before that.
pub fn relative(at: Timestamp, now: Timestamp, tz: &TimeZone) -> String {
    let (at_z, now_z) = (at.to_zoned(tz.clone()), now.to_zoned(tz.clone()));
    let (day, today) = (at_z.date(), now_z.date());
    let secs = now.as_second() - at.as_second();
    if day == today && secs >= 0 {
        return match secs {
            0..60 => "now".to_owned(),
            60..3600 => format!("{}m", secs / 60),
            _ => format!("{}h", secs / 3600),
        };
    }
    if today.checked_sub(1.day()).is_ok_and(|y| y == day) {
        return "Yesterday".to_owned();
    }
    let monday = today.checked_sub((today.weekday().to_monday_zero_offset() as i64).days()).unwrap_or(today);
    if day >= monday && day < today {
        return weekday(day.weekday()).to_owned();
    }
    if day.year() == today.year() {
        return at_z.strftime("%b %-d").to_string();
    }
    at_z.strftime("%b %-d, %Y").to_string()
}

/// The inspector's longer form: `just now`, `12 minutes ago`, `2 hours ago` today;
/// `Yesterday at 18:42`; `Monday at 11:05` earlier this week; `Nov 8 at 16:20` earlier this
/// year; `Nov 8, 2025 at 16:20` before that.
pub fn long(at: Timestamp, now: Timestamp, tz: &TimeZone) -> String {
    let (at_z, now_z) = (at.to_zoned(tz.clone()), now.to_zoned(tz.clone()));
    let (day, today) = (at_z.date(), now_z.date());
    let secs = now.as_second() - at.as_second();
    let clock = at_z.strftime("%H:%M");
    // A moment after "now" (a fetch while the clock is fixed, a skewed commit) reads as now.
    if secs < 60 {
        return "just now".to_owned();
    }
    if day == today {
        let ago = |n: i64, unit: &str| if n == 1 { format!("1 {unit} ago") } else { format!("{n} {unit}s ago") };
        return match secs {
            ..3600 => ago(secs / 60, "minute"),
            _ => ago(secs / 3600, "hour"),
        };
    }
    if today.checked_sub(1.day()).is_ok_and(|y| y == day) {
        return format!("Yesterday at {clock}");
    }
    let monday = today.checked_sub((today.weekday().to_monday_zero_offset() as i64).days()).unwrap_or(today);
    if day >= monday && day < today {
        return format!("{} at {clock}", at_z.strftime("%A"));
    }
    if day.year() == today.year() {
        return format!("{} at {clock}", at_z.strftime("%b %-d"));
    }
    format!("{} at {clock}", at_z.strftime("%b %-d, %Y"))
}

/// The date and time, compact: `Nov 10, 2026, 18:42`.
pub fn date_time(at: Timestamp, tz: &TimeZone) -> String {
    at.to_zoned(tz.clone()).strftime("%b %-d, %Y, %H:%M").to_string()
}

/// The full date and time, for a tooltip: `Mon, Nov 9, 2026 at 14:05`.
pub fn absolute(at: Timestamp, tz: &TimeZone) -> String {
    at.to_zoned(tz.clone()).strftime("%a, %b %-d, %Y at %H:%M").to_string()
}

/// The inspector's date: `Nov 11, 2026, 10:00`.
pub fn stamp(at: Timestamp, tz: &TimeZone) -> String {
    at.to_zoned(tz.clone()).strftime("%b %-d, %Y, %H:%M").to_string()
}

/// How long ago, in the largest whole unit: `just now`, `12 minutes ago`, `5 hours ago`,
/// `yesterday`, `3 days ago`, `2 weeks ago`, `4 months ago`, `2 years ago`.
pub fn ago(at: Timestamp, now: Timestamp) -> String {
    let secs = now.as_second() - at.as_second();
    let n = |unit: i64| secs / unit;
    let say = |n: i64, unit: &str| if n == 1 { format!("1 {unit} ago") } else { format!("{n} {unit}s ago") };
    match secs {
        ..60 => "just now".to_owned(),
        60..3600 => say(n(60), "minute"),
        3600..86_400 => say(n(3600), "hour"),
        86_400..172_800 => "yesterday".to_owned(),
        172_800..1_209_600 => say(n(86_400), "day"),
        1_209_600..5_184_000 => say(n(604_800), "week"),
        5_184_000..31_536_000 => say(n(2_592_000), "month"),
        _ => say(n(31_536_000), "year"),
    }
}

fn weekday(d: Weekday) -> &'static str {
    match d {
        Weekday::Monday => "Mon",
        Weekday::Tuesday => "Tue",
        Weekday::Wednesday => "Wed",
        Weekday::Thursday => "Thu",
        Weekday::Friday => "Fri",
        Weekday::Saturday => "Sat",
        Weekday::Sunday => "Sun",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_read_relative_this_week_and_absolute_before() {
        let tz = TimeZone::UTC;
        let now: Timestamp = "2026-11-11T15:00:00Z".parse().unwrap(); // a Wednesday
        let at = |s: &str| relative(s.parse().unwrap(), now, &tz);
        assert_eq!(at("2026-11-11T14:59:30Z"), "now");
        assert_eq!(at("2026-11-11T14:48:00Z"), "12m");
        assert_eq!(at("2026-11-11T13:00:00Z"), "2h");
        assert_eq!(at("2026-11-11T00:10:00Z"), "14h", "still today");
        assert_eq!(at("2026-11-10T23:00:00Z"), "Yesterday", "under 24 hours, but yesterday");
        assert_eq!(at("2026-11-09T09:00:00Z"), "Mon");
        assert_eq!(at("2026-11-08T09:00:00Z"), "Nov 8", "Sunday was last week");
        assert_eq!(at("2025-12-31T09:00:00Z"), "Dec 31, 2025");
        let long = |s: &str| long(s.parse().unwrap(), now, &tz);
        assert_eq!(long("2026-11-11T14:59:30Z"), "just now");
        assert_eq!(long("2026-11-14T09:00:00Z"), "just now", "after now");
        assert_eq!(long("2026-11-11T14:58:00Z"), "2 minutes ago");
        assert_eq!(long("2026-11-11T14:00:00Z"), "1 hour ago");
        assert_eq!(long("2026-11-11T13:00:00Z"), "2 hours ago");
        assert_eq!(long("2026-11-10T18:42:00Z"), "Yesterday at 18:42");
        assert_eq!(long("2026-11-09T11:05:00Z"), "Monday at 11:05");
        assert_eq!(long("2026-11-08T16:20:00Z"), "Nov 8 at 16:20");
        assert_eq!(long("2025-12-31T09:00:00Z"), "Dec 31, 2025 at 09:00");
        assert_eq!(date_time("2026-11-10T18:42:00Z".parse().unwrap(), &tz), "Nov 10, 2026, 18:42");
        assert_eq!(absolute("2026-11-09T14:05:00Z".parse().unwrap(), &tz), "Mon, Nov 9, 2026 at 14:05");
        assert_eq!(stamp("2026-11-11T10:00:00Z".parse().unwrap(), &tz), "Nov 11, 2026, 10:00");
        let ago = |s: &str| ago(s.parse().unwrap(), now);
        assert_eq!(ago("2026-11-11T14:59:30Z"), "just now");
        assert_eq!(ago("2026-11-11T14:59:00Z"), "1 minute ago");
        assert_eq!(ago("2026-11-11T10:00:00Z"), "5 hours ago");
        assert_eq!(ago("2026-11-10T09:00:00Z"), "yesterday");
        assert_eq!(ago("2026-11-08T15:00:00Z"), "3 days ago");
        assert_eq!(ago("2026-10-28T15:00:00Z"), "2 weeks ago");
        assert_eq!(ago("2026-07-01T15:00:00Z"), "4 months ago");
        assert_eq!(ago("2024-11-01T15:00:00Z"), "2 years ago");
    }
}
