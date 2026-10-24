//! Working-time arithmetic on one calendar. Instants are minutes since 1970-01-01 00:00.
use crate::model::{Calendar, STD_HOURS};
use chrono::{Datelike, Days, NaiveDate};
use std::collections::HashMap;

pub const DAY: i64 = 1440;

fn epoch() -> NaiveDate {
    NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()
}

/// Midnight at the start of `d`.
pub fn at(d: NaiveDate) -> i64 {
    (d - epoch()).num_days() * DAY
}

pub fn date_of(t: i64) -> NaiveDate {
    let n = t.div_euclid(DAY);
    if n >= 0 { epoch() + Days::new(n as u64) } else { epoch() - Days::new((-n) as u64) }
}

type Iv = Vec<(i64, i64)>;

pub struct WorkCal {
    week: [Iv; 7],
    exc: HashMap<NaiveDate, Iv>,
}

fn iv(h: &[(u16, u16)]) -> Iv {
    let mut v: Iv = h.iter().filter(|(s, e)| e > s).map(|&(s, e)| (s as i64, (e as i64).min(DAY))).collect();
    v.sort_unstable();
    v
}

impl WorkCal {
    pub fn new(c: &Calendar) -> Self {
        let mut week: [Iv; 7] = std::array::from_fn(|i| iv(&c.week[i]));
        if week.iter().all(Vec::is_empty) {
            week = std::array::from_fn(|i| if i < 5 { iv(&STD_HOURS) } else { vec![] });
        }
        let mut exc = HashMap::new();
        for e in &c.exceptions {
            let mut d = e.from;
            let mut n = 0;
            while d <= e.to && n < 3700 {
                exc.insert(d, iv(&e.hours));
                d = d.succ_opt().unwrap();
                n += 1;
            }
        }
        Self { week, exc }
    }

    pub fn day(&self, d: NaiveDate) -> &[(i64, i64)] {
        self.exc.get(&d).unwrap_or(&self.week[d.weekday().num_days_from_monday() as usize])
    }

    pub fn is_work_day(&self, d: NaiveDate) -> bool {
        !self.day(d).is_empty()
    }

    /// Moves forward `w` working minutes from `t`. A positive move can end exactly at the end of a shift.
    pub fn add(&self, t: i64, mut w: i64) -> i64 {
        if w <= 0 {
            return if w == 0 { t } else { self.sub(t, -w) };
        }
        let (mut d, mut m) = (date_of(t), t.rem_euclid(DAY));
        loop {
            for &(s, e) in self.day(d) {
                if e <= m {
                    continue;
                }
                let st = s.max(m);
                if w <= e - st {
                    return at(d) + st + w;
                }
                w -= e - st;
            }
            d = d.succ_opt().unwrap();
            m = 0;
        }
    }

    /// Moves back `w` working minutes from `t`.
    pub fn sub(&self, t: i64, mut w: i64) -> i64 {
        if w <= 0 {
            return if w == 0 { t } else { self.add(t, -w) };
        }
        let (mut d, mut m) = (date_of(t), t.rem_euclid(DAY));
        loop {
            for &(s, e) in self.day(d).iter().rev() {
                if s >= m {
                    continue;
                }
                let en = e.min(m);
                if w <= en - s {
                    return at(d) + en - w;
                }
                w -= en - s;
            }
            d = d.pred_opt().unwrap();
            m = DAY;
        }
    }

    /// Working minutes in [a, b); negative when b is before a.
    pub fn between(&self, a: i64, b: i64) -> i64 {
        if b < a {
            return -self.between(b, a);
        }
        let (mut d, last) = (date_of(a), date_of(b));
        let mut n = 0;
        while d <= last {
            let base = at(d);
            for &(s, e) in self.day(d) {
                n += ((base + e).min(b) - (base + s).max(a)).max(0);
            }
            d = d.succ_opt().unwrap();
        }
        n
    }

    /// The next instant at or after `t` that is working time.
    pub fn next_start(&self, t: i64) -> i64 {
        let (mut d, mut m) = (date_of(t), t.rem_euclid(DAY));
        loop {
            if let Some(&(s, _)) = self.day(d).iter().find(|&&(_, e)| m < e) {
                return at(d) + s.max(m);
            }
            d = d.succ_opt().unwrap();
            m = 0;
        }
    }

    /// The last instant at or before `t` that ends working time.
    pub fn prev_end(&self, t: i64) -> i64 {
        let (mut d, mut m) = (date_of(t), t.rem_euclid(DAY));
        if m == 0 {
            d = d.pred_opt().unwrap();
            m = DAY;
        }
        loop {
            if let Some(&(_, e)) = self.day(d).iter().rev().find(|&&(s, _)| m > s) {
                return at(d) + e.min(m);
            }
            d = d.pred_opt().unwrap();
            m = DAY;
        }
    }

    /// Is `t` inside working time or exactly at the end of a shift?
    pub fn in_or_end(&self, t: i64) -> bool {
        let (d, m) = (date_of(t), t.rem_euclid(DAY));
        self.day(d).iter().any(|&(s, e)| m >= s && m <= e) || (m == 0 && self.day(d.pred_opt().unwrap()).last().is_some_and(|&(_, e)| e == DAY))
    }

    /// End of the working day `d` (or of the last working day before it).
    pub fn end_of(&self, d: NaiveDate) -> i64 {
        self.prev_end(at(d) + DAY)
    }

    /// Working minutes on day `d`.
    pub fn day_minutes(&self, d: NaiveDate) -> i64 {
        self.day(d).iter().map(|(s, e)| e - s).sum()
    }
}
