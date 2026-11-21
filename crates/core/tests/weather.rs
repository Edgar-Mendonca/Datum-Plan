//! Weather in schedule risk (0.10.1): working days lost by month on a calendar.
//! Plans start Monday 2 Nov 2026 on the standard calendar (8 h, Mon–Fri).
use chrono::NaiveDate;
use datum_core::{cal::at, risk::{simulate, SimOpts}, *};

fn d(y: i32, m: u32, day: u32) -> NaiveDate { NaiveDate::from_ymd_opt(y, m, day).unwrap() }
fn opts(runs: u32) -> SimOpts { SimOpts { iterations: runs, low: 0.0, high: 0.0, seed: 3, ..Default::default() } }
fn ends(day: NaiveDate) -> i64 { at(day) + 17 * 60 }
/// One 5-day task on the project calendar, with that calendar's weather (% lost, January first).
fn plan(weather: &[(usize, f64)]) -> Project {
    let mut p = Project::new("W", d(2026, 11, 2));
    let t = p.new_task("Earthworks", 0, 5 * 480);
    p.tasks.push(t);
    let k = p.calendars.iter().position(|c| c.name == p.calendar).unwrap();
    if !weather.is_empty() {
        p.calendars[k].weather = vec![0.0; 12];
        for &(m, w) in weather { p.calendars[k].weather[m] = w }
    }
    p
}

/// Every November day lost: the work waits for Tue 1 Dec and takes five days, to Mon 7 Dec.
/// The five working days lost up to the planned finish (Mon 2 – Fri 6 Nov) are reported.
#[test]
fn a_month_lost_entirely() {
    let r = simulate(&plan(&[(10, 100.0)]), &opts(20)).unwrap();
    assert_eq!(r.planned, ends(d(2026, 11, 6)));
    assert_eq!((r.p10, r.p90), (ends(d(2026, 12, 7)), ends(d(2026, 12, 7))));
    assert_eq!(r.weather_days, 5.0);
    assert!(r.notes.iter().any(|n| n.starts_with("Weather:")));
}

/// No weather, or none in the months the work falls in: every run is the plan.
#[test]
fn no_weather_changes_nothing() {
    for p in [plan(&[]), plan(&[(0, 50.0), (1, 50.0)])] {
        let r = simulate(&p, &opts(20)).unwrap();
        assert_eq!((r.p10, r.p90), (ends(d(2026, 11, 6)), ends(d(2026, 11, 6))));
    }
    assert!(simulate(&plan(&[]), &opts(20)).unwrap().notes.iter().all(|n| !n.starts_with("Weather:")));
}

/// Half the November days lost: some runs finish later, never earlier; about 2.5 days lost a run.
#[test]
fn half_the_days_lost() {
    let r = simulate(&plan(&[(10, 50.0)]), &opts(400)).unwrap();
    assert!(r.p10 >= r.planned && r.p90 > r.p50 && r.p50 > r.planned, "{} {} {}", r.p10, r.p50, r.p90);
    assert!((1.5..=3.5).contains(&r.weather_days), "{}", r.weather_days);
}

/// Weather on a calendar no task uses moves nothing.
#[test]
fn weather_on_an_unused_calendar() {
    let mut p = plan(&[]);
    let mut c = p.calendars[0].clone();
    c.name = "Outdoor".into();
    c.weather = vec![100.0; 12];
    p.calendars.push(c);
    let r = simulate(&p, &opts(20)).unwrap();
    assert_eq!(r.p90, ends(d(2026, 11, 6)));
}
