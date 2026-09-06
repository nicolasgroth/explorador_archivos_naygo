// Naygo — pruebas del calendario local y transiciones de horario de verano.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT
use super::*;
use naygo_core::saved_search::RelativeDate;
#[test]
fn calendar_dates_use_the_requested_local_zone() {
    let zone = chrono::FixedOffset::west_opt(4 * 3600).unwrap();
    assert_eq!(
        search_date_in_zone(RelativeDate::Today, 12 * 3600, &zone),
        Some(4 * 3600)
    );
    assert_eq!(
        search_date_in_zone(RelativeDate::ThisWeek, 12 * 3600, &zone),
        Some(-3 * 86400 + 4 * 3600)
    );
    assert_eq!(
        search_date_in_zone(RelativeDate::Last7Days, 12 * 3600, &zone),
        Some(12 * 3600 - 7 * 86400)
    );
    assert_eq!(
        search_date_in_zone(RelativeDate::Any, 12 * 3600, &zone),
        None
    );
}
#[test]
fn month_uses_start_of_local_month_and_its_historical_offset() {
    use chrono::TimeZone;
    let now = chrono::Utc
        .with_ymd_and_hms(2026, 9, 8, 12, 0, 0)
        .unwrap()
        .timestamp();
    let expected = chrono::Utc
        .with_ymd_and_hms(2026, 9, 1, 4, 0, 0)
        .unwrap()
        .timestamp();
    assert_eq!(
        search_date_in_zone(RelativeDate::ThisMonth, now, &MidnightJump),
        Some(expected)
    );
    let before_local_midnight = chrono::Utc
        .with_ymd_and_hms(2026, 9, 1, 2, 0, 0)
        .unwrap()
        .timestamp();
    let august = chrono::Utc
        .with_ymd_and_hms(2026, 8, 1, 4, 0, 0)
        .unwrap()
        .timestamp();
    assert_eq!(
        search_date_in_zone(
            RelativeDate::ThisMonth,
            before_local_midnight,
            &MidnightJump
        ),
        Some(august)
    );
}
#[test]
fn invalid_timestamp_is_not_converted_into_an_unbounded_calendar_query() {
    assert_eq!(
        search_date_in_zone(RelativeDate::Today, i64::MAX, &chrono::Utc),
        None
    );
}

/// Zona sintética: el domingo 2026-09-06 salta de 00:00 a 01:00 (UTC-4 → UTC-3).
#[derive(Clone)]
struct MidnightJump;
impl chrono::TimeZone for MidnightJump {
    type Offset = chrono::FixedOffset;
    fn from_offset(_: &Self::Offset) -> Self {
        Self
    }
    fn offset_from_local_date(
        &self,
        date: &chrono::NaiveDate,
    ) -> chrono::MappedLocalTime<Self::Offset> {
        self.offset_from_local_datetime(&date.and_hms_opt(12, 0, 0).unwrap())
    }
    fn offset_from_local_datetime(
        &self,
        date: &chrono::NaiveDateTime,
    ) -> chrono::MappedLocalTime<Self::Offset> {
        let transition = chrono::NaiveDate::from_ymd_opt(2026, 9, 6)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        if *date >= transition && *date < transition + chrono::TimeDelta::hours(1) {
            return chrono::MappedLocalTime::None;
        }
        chrono::MappedLocalTime::Single(
            chrono::FixedOffset::west_opt(if *date < transition {
                4 * 3600
            } else {
                3 * 3600
            })
            .unwrap(),
        )
    }
    fn offset_from_utc_date(&self, date: &chrono::NaiveDate) -> Self::Offset {
        self.offset_from_utc_datetime(&date.and_hms_opt(12, 0, 0).unwrap())
    }
    fn offset_from_utc_datetime(&self, date: &chrono::NaiveDateTime) -> Self::Offset {
        let transition = chrono::NaiveDate::from_ymd_opt(2026, 9, 6)
            .unwrap()
            .and_hms_opt(4, 0, 0)
            .unwrap();
        chrono::FixedOffset::west_opt(if *date < transition {
            4 * 3600
        } else {
            3 * 3600
        })
        .unwrap()
    }
}
#[test]
fn week_uses_mondays_offset_and_today_skips_nonexistent_midnight() {
    use chrono::TimeZone;
    let now = chrono::Utc
        .with_ymd_and_hms(2026, 9, 6, 12, 0, 0)
        .unwrap()
        .timestamp();
    let monday = chrono::Utc
        .with_ymd_and_hms(2026, 8, 31, 4, 0, 0)
        .unwrap()
        .timestamp();
    let sunday = chrono::Utc
        .with_ymd_and_hms(2026, 9, 6, 4, 0, 0)
        .unwrap()
        .timestamp();
    assert_eq!(
        search_date_in_zone(RelativeDate::ThisWeek, now, &MidnightJump),
        Some(monday)
    );
    assert_eq!(
        search_date_in_zone(RelativeDate::Today, now, &MidnightJump),
        Some(sunday)
    );
}
