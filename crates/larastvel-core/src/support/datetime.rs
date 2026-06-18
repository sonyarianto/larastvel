use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, Timelike, Utc};
use std::cmp::Ordering;
use std::ops::{Add, Sub};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dt(DateTime<Utc>);

impl Dt {
    pub fn now() -> Self {
        Self(Utc::now())
    }

    pub fn today() -> Self {
        let now = Utc::now();
        Self(DateTime::from_naive_utc_and_offset(
            now.date_naive().and_hms_opt(0, 0, 0).unwrap(),
            Utc,
        ))
    }

    pub fn tomorrow() -> Self {
        Self::today().add_days(1)
    }

    pub fn yesterday() -> Self {
        Self::today().sub_days(1)
    }

    pub fn from_naive_utc(dt: NaiveDateTime) -> Self {
        Self(DateTime::from_naive_utc_and_offset(dt, Utc))
    }

    pub fn from_ymd(year: i32, month: u32, day: u32) -> Self {
        let date = NaiveDate::from_ymd_opt(year, month, day).unwrap();
        Self(DateTime::from_naive_utc_and_offset(
            date.and_hms_opt(0, 0, 0).unwrap(),
            Utc,
        ))
    }

    pub fn from_ymd_hms(year: i32, month: u32, day: u32, hour: u32, min: u32, sec: u32) -> Self {
        let date = NaiveDate::from_ymd_opt(year, month, day).unwrap();
        Self(DateTime::from_naive_utc_and_offset(
            date.and_hms_opt(hour, min, sec).unwrap(),
            Utc,
        ))
    }

    pub fn parse(s: &str) -> Option<Self> {
        s.parse::<DateTime<Utc>>().ok().map(Self).or_else(|| {
            NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
                .ok()
                .map(|dt| Self(DateTime::from_naive_utc_and_offset(dt, Utc)))
        })
    }

    pub fn from_format(s: &str, fmt: &str) -> Option<Self> {
        NaiveDateTime::parse_from_str(s, fmt)
            .ok()
            .map(|dt| Self(DateTime::from_naive_utc_and_offset(dt, Utc)))
    }

    pub fn format(&self, fmt: &str) -> String {
        self.0.format(fmt).to_string()
    }

    pub fn to_date_string(&self) -> String {
        self.0.format("%Y-%m-%d").to_string()
    }

    pub fn to_time_string(&self) -> String {
        self.0.format("%H:%M:%S").to_string()
    }

    pub fn to_datetime_string(&self) -> String {
        self.0.format("%Y-%m-%d %H:%M:%S").to_string()
    }

    pub fn to_rfc3339(&self) -> String {
        self.0.to_rfc3339()
    }

    pub fn to_rfc2822(&self) -> String {
        self.0.to_rfc2822()
    }

    pub fn to_iso_string(&self) -> String {
        self.0.to_rfc3339()
    }

    pub fn timestamp(&self) -> i64 {
        self.0.timestamp()
    }

    pub fn to_naive_utc(&self) -> NaiveDateTime {
        self.0.naive_utc()
    }

    pub fn year(&self) -> i32 {
        self.0.year()
    }

    pub fn month(&self) -> u32 {
        self.0.month()
    }

    pub fn day(&self) -> u32 {
        self.0.day()
    }

    pub fn hour(&self) -> u32 {
        self.0.hour()
    }

    pub fn minute(&self) -> u32 {
        self.0.minute()
    }

    pub fn second(&self) -> u32 {
        self.0.second()
    }

    pub fn add_days(self, days: i64) -> Self {
        Self(self.0 + Duration::days(days))
    }

    pub fn sub_days(self, days: i64) -> Self {
        Self(self.0 - Duration::days(days))
    }

    pub fn add_hours(self, hours: i64) -> Self {
        Self(self.0 + Duration::hours(hours))
    }

    pub fn sub_hours(self, hours: i64) -> Self {
        Self(self.0 - Duration::hours(hours))
    }

    pub fn add_minutes(self, minutes: i64) -> Self {
        Self(self.0 + Duration::minutes(minutes))
    }

    pub fn sub_minutes(self, minutes: i64) -> Self {
        Self(self.0 - Duration::minutes(minutes))
    }

    pub fn add_seconds(self, seconds: i64) -> Self {
        Self(self.0 + Duration::seconds(seconds))
    }

    pub fn sub_seconds(self, seconds: i64) -> Self {
        Self(self.0 - Duration::seconds(seconds))
    }

    pub fn add_weeks(self, weeks: i64) -> Self {
        Self(self.0 + Duration::weeks(weeks))
    }

    pub fn sub_weeks(self, weeks: i64) -> Self {
        Self(self.0 - Duration::weeks(weeks))
    }

    pub fn start_of_day(&self) -> Self {
        let dt = self.0;
        Self(DateTime::from_naive_utc_and_offset(
            dt.date_naive().and_hms_opt(0, 0, 0).unwrap(),
            Utc,
        ))
    }

    pub fn end_of_day(&self) -> Self {
        let dt = self.0;
        Self(DateTime::from_naive_utc_and_offset(
            dt.date_naive().and_hms_opt(23, 59, 59).unwrap(),
            Utc,
        ))
    }

    pub fn start_of_month(&self) -> Self {
        let dt = self.0;
        Self(DateTime::from_naive_utc_and_offset(
            NaiveDate::from_ymd_opt(dt.year(), dt.month(), 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap(),
            Utc,
        ))
    }

    pub fn end_of_month(&self) -> Self {
        let dt = self.0;
        let last_day = num_days_in_month(dt.year(), dt.month());
        Self(DateTime::from_naive_utc_and_offset(
            NaiveDate::from_ymd_opt(dt.year(), dt.month(), last_day)
                .unwrap()
                .and_hms_opt(23, 59, 59)
                .unwrap(),
            Utc,
        ))
    }

    pub fn start_of_year(&self) -> Self {
        let dt = self.0;
        Self(DateTime::from_naive_utc_and_offset(
            NaiveDate::from_ymd_opt(dt.year(), 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap(),
            Utc,
        ))
    }

    pub fn end_of_year(&self) -> Self {
        let dt = self.0;
        Self(DateTime::from_naive_utc_and_offset(
            NaiveDate::from_ymd_opt(dt.year(), 12, 31)
                .unwrap()
                .and_hms_opt(23, 59, 59)
                .unwrap(),
            Utc,
        ))
    }

    pub fn start_of_week(&self) -> Self {
        let dt = self.0;
        let days_from_monday = dt.weekday().num_days_from_monday();
        Self(DateTime::from_naive_utc_and_offset(
            (dt.date_naive() - Duration::days(days_from_monday as i64))
                .and_hms_opt(0, 0, 0)
                .unwrap(),
            Utc,
        ))
    }

    pub fn end_of_week(&self) -> Self {
        let dt = self.0;
        let days_from_monday = dt.weekday().num_days_from_monday();
        Self(DateTime::from_naive_utc_and_offset(
            (dt.date_naive() + Duration::days(6 - days_from_monday as i64))
                .and_hms_opt(23, 59, 59)
                .unwrap(),
            Utc,
        ))
    }

    pub fn diff_in_days(&self, other: &Self) -> i64 {
        (self.0 - other.0).num_days()
    }

    pub fn diff_in_hours(&self, other: &Self) -> i64 {
        (self.0 - other.0).num_hours()
    }

    pub fn diff_in_minutes(&self, other: &Self) -> i64 {
        (self.0 - other.0).num_minutes()
    }

    pub fn diff_in_seconds(&self, other: &Self) -> i64 {
        (self.0 - other.0).num_seconds()
    }

    pub fn diff_in_weeks(&self, other: &Self) -> i64 {
        (self.0 - other.0).num_weeks()
    }

    pub fn gt(&self, other: &Self) -> bool {
        self.0 > other.0
    }

    pub fn gte(&self, other: &Self) -> bool {
        self.0 >= other.0
    }

    pub fn lt(&self, other: &Self) -> bool {
        self.0 < other.0
    }

    pub fn lte(&self, other: &Self) -> bool {
        self.0 <= other.0
    }

    pub fn is_future(&self) -> bool {
        self.0 > Utc::now()
    }

    pub fn is_past(&self) -> bool {
        self.0 < Utc::now()
    }

    pub fn is_today(&self) -> bool {
        self.0.date_naive() == Utc::now().date_naive()
    }

    pub fn is_weekend(&self) -> bool {
        matches!(
            self.0.weekday(),
            chrono::Weekday::Sat | chrono::Weekday::Sun
        )
    }

    pub fn is_weekday(&self) -> bool {
        !self.is_weekend()
    }

    pub fn average(a: &Self, b: &Self) -> Self {
        let diff = (b.0 - a.0).num_milliseconds() / 2;
        Self(a.0 + Duration::milliseconds(diff))
    }

    pub fn copy(&self) -> Self {
        *self
    }

    pub fn inner(&self) -> &DateTime<Utc> {
        &self.0
    }

    pub fn into_inner(self) -> DateTime<Utc> {
        self.0
    }
}

impl Add<Duration> for Dt {
    type Output = Self;

    fn add(self, rhs: Duration) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl Sub<Duration> for Dt {
    type Output = Self;

    fn sub(self, rhs: Duration) -> Self::Output {
        Self(self.0 - rhs)
    }
}

impl PartialOrd for Dt {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Dt {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl std::fmt::Display for Dt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_datetime_string())
    }
}

impl From<DateTime<Utc>> for Dt {
    fn from(dt: DateTime<Utc>) -> Self {
        Self(dt)
    }
}

impl From<Dt> for DateTime<Utc> {
    fn from(dt: Dt) -> Self {
        dt.0
    }
}

impl From<NaiveDateTime> for Dt {
    fn from(dt: NaiveDateTime) -> Self {
        Self(DateTime::from_naive_utc_and_offset(dt, Utc))
    }
}

fn num_days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

pub fn now() -> Dt {
    Dt::now()
}

pub fn today() -> Dt {
    Dt::today()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_now() {
        let dt = Dt::now();
        assert!(!dt.is_future());
        assert!(!dt.is_past() || dt.is_past());
    }

    #[test]
    fn test_today() {
        let dt = Dt::today();
        assert_eq!(dt.hour(), 0);
        assert_eq!(dt.minute(), 0);
        assert_eq!(dt.second(), 0);
    }

    #[test]
    fn test_tomorrow_yesterday() {
        let tom = Dt::tomorrow();
        let yes = Dt::yesterday();
        assert!(tom.gt(&yes));
        assert!(yes.lt(&tom));
    }

    #[test]
    fn test_from_ymd() {
        let dt = Dt::from_ymd(2025, 6, 15);
        assert_eq!(dt.year(), 2025);
        assert_eq!(dt.month(), 6);
        assert_eq!(dt.day(), 15);
    }

    #[test]
    fn test_from_ymd_hms() {
        let dt = Dt::from_ymd_hms(2025, 6, 15, 10, 30, 0);
        assert_eq!(dt.hour(), 10);
        assert_eq!(dt.minute(), 30);
    }

    #[test]
    fn test_parse_iso() {
        let dt = Dt::parse("2025-06-15T10:30:00Z").unwrap();
        assert_eq!(dt.year(), 2025);
        assert_eq!(dt.month(), 6);
        assert_eq!(dt.day(), 15);
    }

    #[test]
    fn test_parse_space_separated() {
        let dt = Dt::parse("2025-06-15 10:30:00").unwrap();
        assert_eq!(dt.hour(), 10);
    }

    #[test]
    fn test_parse_invalid() {
        assert!(Dt::parse("not-a-date").is_none());
    }

    #[test]
    fn test_from_format() {
        let dt = Dt::from_format("15/06/2025 10:30", "%d/%m/%Y %H:%M").unwrap();
        assert_eq!(dt.year(), 2025);
        assert_eq!(dt.month(), 6);
        assert_eq!(dt.day(), 15);
    }

    #[test]
    fn test_format() {
        let dt = Dt::from_ymd(2025, 6, 15);
        assert_eq!(dt.format("%Y-%m-%d"), "2025-06-15");
    }

    #[test]
    fn test_to_date_string() {
        let dt = Dt::from_ymd(2025, 6, 15);
        assert_eq!(dt.to_date_string(), "2025-06-15");
    }

    #[test]
    fn test_to_datetime_string() {
        let dt = Dt::from_ymd_hms(2025, 6, 15, 8, 5, 3);
        assert_eq!(dt.to_datetime_string(), "2025-06-15 08:05:03");
    }

    #[test]
    fn test_timestamp() {
        let dt = Dt::from_ymd(2025, 1, 1);
        assert!(dt.timestamp() > 0);
    }

    #[test]
    fn test_add_days() {
        let dt = Dt::from_ymd(2025, 1, 1).add_days(10);
        assert_eq!(dt.day(), 11);
    }

    #[test]
    fn test_sub_days() {
        let dt = Dt::from_ymd(2025, 1, 15).sub_days(5);
        assert_eq!(dt.day(), 10);
    }

    #[test]
    fn test_add_hours() {
        let dt = Dt::from_ymd_hms(2025, 1, 1, 10, 0, 0).add_hours(5);
        assert_eq!(dt.hour(), 15);
    }

    #[test]
    fn test_add_minutes() {
        let dt = Dt::from_ymd_hms(2025, 1, 1, 10, 30, 0).add_minutes(15);
        assert_eq!(dt.minute(), 45);
    }

    #[test]
    fn test_add_seconds() {
        let dt = Dt::from_ymd_hms(2025, 1, 1, 10, 30, 0).add_seconds(45);
        assert_eq!(dt.second(), 45);
    }

    #[test]
    fn test_add_weeks() {
        let dt = Dt::from_ymd(2025, 1, 1).add_weeks(2);
        assert_eq!(dt.day(), 15);
    }

    #[test]
    fn test_start_of_day() {
        let dt = Dt::from_ymd_hms(2025, 6, 15, 14, 30, 45).start_of_day();
        assert_eq!(dt.hour(), 0);
        assert_eq!(dt.minute(), 0);
        assert_eq!(dt.second(), 0);
    }

    #[test]
    fn test_end_of_day() {
        let dt = Dt::from_ymd_hms(2025, 6, 15, 14, 30, 45).end_of_day();
        assert_eq!(dt.hour(), 23);
        assert_eq!(dt.minute(), 59);
        assert_eq!(dt.second(), 59);
    }

    #[test]
    fn test_start_of_month() {
        let dt = Dt::from_ymd(2025, 6, 15).start_of_month();
        assert_eq!(dt.day(), 1);
    }

    #[test]
    fn test_end_of_month() {
        let dt = Dt::from_ymd(2025, 6, 15).end_of_month();
        assert_eq!(dt.day(), 30);
    }

    #[test]
    fn test_end_of_month_feb_leap() {
        let dt = Dt::from_ymd(2024, 2, 10).end_of_month();
        assert_eq!(dt.day(), 29);
    }

    #[test]
    fn test_end_of_month_feb_non_leap() {
        let dt = Dt::from_ymd(2025, 2, 10).end_of_month();
        assert_eq!(dt.day(), 28);
    }

    #[test]
    fn test_start_of_year() {
        let dt = Dt::from_ymd(2025, 6, 15).start_of_year();
        assert_eq!(dt.month(), 1);
        assert_eq!(dt.day(), 1);
    }

    #[test]
    fn test_end_of_year() {
        let dt = Dt::from_ymd(2025, 6, 15).end_of_year();
        assert_eq!(dt.month(), 12);
        assert_eq!(dt.day(), 31);
    }

    #[test]
    fn test_diff_in_days() {
        let a = Dt::from_ymd(2025, 1, 10);
        let b = Dt::from_ymd(2025, 1, 1);
        assert_eq!(a.diff_in_days(&b), 9);
    }

    #[test]
    fn test_diff_in_hours() {
        let a = Dt::from_ymd_hms(2025, 1, 1, 12, 0, 0);
        let b = Dt::from_ymd_hms(2025, 1, 1, 10, 0, 0);
        assert_eq!(a.diff_in_hours(&b), 2);
    }

    #[test]
    fn test_diff_in_minutes() {
        let a = Dt::from_ymd_hms(2025, 1, 1, 10, 30, 0);
        let b = Dt::from_ymd_hms(2025, 1, 1, 10, 15, 0);
        assert_eq!(a.diff_in_minutes(&b), 15);
    }

    #[test]
    fn test_diff_in_seconds() {
        let a = Dt::from_ymd_hms(2025, 1, 1, 10, 0, 30);
        let b = Dt::from_ymd_hms(2025, 1, 1, 10, 0, 0);
        assert_eq!(a.diff_in_seconds(&b), 30);
    }

    #[test]
    fn test_diff_in_weeks() {
        let a = Dt::from_ymd(2025, 1, 15);
        let b = Dt::from_ymd(2025, 1, 1);
        assert_eq!(a.diff_in_weeks(&b), 2);
    }

    #[test]
    fn test_comparison() {
        let a = Dt::from_ymd(2025, 1, 1);
        let b = Dt::from_ymd(2025, 1, 2);
        assert!(a.lt(&b));
        assert!(b.gt(&a));
        assert!(a.lte(&b));
        assert!(b.gte(&a));
    }

    #[test]
    fn test_is_weekend() {
        let sat = Dt::from_ymd(2025, 6, 14); // Saturday
        let sun = Dt::from_ymd(2025, 6, 15); // Sunday
        let mon = Dt::from_ymd(2025, 6, 16); // Monday
        assert!(sat.is_weekend());
        assert!(sun.is_weekend());
        assert!(!mon.is_weekend());
        assert!(mon.is_weekday());
    }

    #[test]
    fn test_is_today() {
        assert!(Dt::now().is_today());
        assert!(!Dt::from_ymd(2020, 1, 1).is_today());
    }

    #[test]
    fn test_average() {
        let a = Dt::from_ymd(2025, 1, 1);
        let b = Dt::from_ymd(2025, 1, 3);
        let avg = Dt::average(&a, &b);
        assert_eq!(avg.day(), 2);
    }

    #[test]
    fn test_display() {
        let dt = Dt::from_ymd_hms(2025, 6, 15, 10, 30, 0);
        let s = format!("{}", dt);
        assert!(s.contains("2025-06-15 10:30:00"));
    }

    #[test]
    fn test_from_chrono_datetime() {
        let chrono_dt = Utc::now();
        let dt = Dt::from(chrono_dt);
        assert_eq!(dt.timestamp(), chrono_dt.timestamp());
    }

    #[test]
    fn test_into_chrono_datetime() {
        let dt = Dt::now();
        let chrono_dt: DateTime<Utc> = dt.into();
        assert_eq!(dt.timestamp(), chrono_dt.timestamp());
    }

    #[test]
    fn test_from_naive_datetime() {
        let naive = NaiveDateTime::new(
            NaiveDate::from_ymd_opt(2025, 6, 15).unwrap(),
            chrono::NaiveTime::from_hms_opt(10, 30, 0).unwrap(),
        );
        let dt = Dt::from(naive);
        assert_eq!(dt.year(), 2025);
        assert_eq!(dt.month(), 6);
        assert_eq!(dt.day(), 15);
    }

    #[test]
    fn test_add_duration() {
        let dt = Dt::from_ymd(2025, 1, 1) + Duration::days(10);
        assert_eq!(dt.day(), 11);
    }

    #[test]
    fn test_sub_duration() {
        let dt = Dt::from_ymd(2025, 1, 15) - Duration::days(5);
        assert_eq!(dt.day(), 10);
    }

    #[test]
    fn test_ordering() {
        let a = Dt::from_ymd(2025, 1, 1);
        let b = Dt::from_ymd(2025, 1, 2);
        let c = Dt::from_ymd(2025, 1, 2);
        assert!(a < b);
        assert!(b > a);
        assert!(b >= c);
        assert!(a <= b);
    }

    #[test]
    fn test_free_functions() {
        let dt = now();
        assert!(dt.timestamp() > 0);
        let t = today();
        assert_eq!(t.hour(), 0);
    }

    #[test]
    fn test_start_of_week() {
        let wed = Dt::from_ymd(2025, 6, 18); // Wednesday
        let start = wed.start_of_week();
        assert_eq!(start.day(), 16); // Monday
    }

    #[test]
    fn test_end_of_week() {
        let wed = Dt::from_ymd(2025, 6, 18); // Wednesday
        let end = wed.end_of_week();
        assert_eq!(end.day(), 22); // Sunday
    }

    #[test]
    fn test_copy() {
        let a = Dt::from_ymd(2025, 1, 1);
        let b = a.copy();
        assert_eq!(a, b);
    }

    #[test]
    fn test_inner_into_inner() {
        let dt = Dt::now();
        let inner: &DateTime<Utc> = dt.inner();
        let _consumed: DateTime<Utc> = dt.into_inner();
        assert!(inner.timestamp() > 0);
    }

    #[test]
    fn test_serialization_implied() {
        // chrono's serde is enabled; Dt wraps it, but doesn't derive Serialize itself
        let dt = Dt::now();
        let json = serde_json::to_string(&dt.inner()).unwrap();
        assert!(json.contains("T"));
    }

    #[test]
    fn test_is_past_future() {
        let past = Dt::from_ymd(2020, 1, 1);
        let future = Dt::from_ymd(2030, 1, 1);
        assert!(past.is_past());
        assert!(future.is_future());
    }

    #[test]
    fn test_to_rfc3339() {
        let dt = Dt::from_ymd_hms(2025, 6, 15, 10, 30, 0);
        assert!(dt.to_rfc3339().contains("10:30:00"));
    }

    #[test]
    fn test_to_rfc2822() {
        let dt = Dt::from_ymd(2025, 6, 15);
        assert!(dt.to_rfc2822().contains("Jun 2025"));
    }
}
