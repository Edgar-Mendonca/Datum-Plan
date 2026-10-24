//! Working-day arithmetic. Scheduling runs on working-day ordinals: 0 is the first
//! working day on or after the project start.
use crate::model::Calendar;
use chrono::{Datelike, NaiveDate};
use std::collections::HashSet;

pub struct WorkCal {
    week: [bool; 7],
    hol: HashSet<NaiveDate>,
    days: Vec<NaiveDate>,
}

impl WorkCal {
    pub fn new(cal: &Calendar, start: NaiveDate) -> Self {
        let week = if cal.week.iter().any(|&w| w) { cal.week } else { Calendar::default().week };
        let mut c = Self { week, hol: cal.holidays.iter().copied().collect(), days: vec![] };
        let mut d = start;
        while !c.is_work(d) {
            d = d.succ_opt().unwrap();
        }
        c.days.push(d);
        c
    }

    pub fn is_work(&self, d: NaiveDate) -> bool {
        self.week[d.weekday().num_days_from_monday() as usize] && !self.hol.contains(&d)
    }

    fn grow_to(&mut self, k: usize) {
        while self.days.len() <= k {
            let mut d = self.days.last().unwrap().succ_opt().unwrap();
            while !self.is_work(d) {
                d = d.succ_opt().unwrap();
            }
            self.days.push(d);
        }
    }

    /// Date of working-day ordinal `k`.
    pub fn date(&mut self, k: i64) -> NaiveDate {
        if k >= 0 {
            self.grow_to(k as usize);
            return self.days[k as usize];
        }
        let mut d = self.days[0];
        let mut n = k;
        while n < 0 {
            d = d.pred_opt().unwrap();
            if self.is_work(d) {
                n += 1;
            }
        }
        d
    }

    /// Ordinal of the first working day on or after `d`.
    pub fn ord(&mut self, d: NaiveDate) -> i64 {
        let first = self.days[0];
        if d <= first {
            let (mut x, mut n) = (d, 0);
            while x < first {
                if self.is_work(x) {
                    n -= 1;
                }
                x = x.succ_opt().unwrap();
            }
            return n;
        }
        while *self.days.last().unwrap() < d {
            let n = self.days.len();
            self.grow_to(n);
        }
        self.days.partition_point(|x| *x < d) as i64
    }
}
