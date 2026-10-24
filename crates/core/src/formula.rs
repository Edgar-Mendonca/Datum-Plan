//! Custom field formulas in MS Project syntax: `IIf([Finish] > [Baseline Finish], "Late", "On time")`.
//! Durations are minutes, dates are instants (minutes); dates subtract to days like VBA.
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum F { Num(f64), Str(String), Bool(bool), Date(i64), Null }

impl fmt::Display for F {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            F::Num(n) => write!(f, "{}", if n.fract() == 0.0 { format!("{}", *n as i64) } else { format!("{n:.2}") }),
            F::Str(s) => write!(f, "{s}"),
            F::Bool(b) => write!(f, "{}", if *b { "Yes" } else { "No" }),
            F::Date(d) => write!(f, "{}", crate::cal::date_of(*d)),
            F::Null => Ok(()),
        }
    }
}

impl F {
    pub fn num(&self) -> f64 {
        match self { F::Num(n) => *n, F::Bool(b) => -(*b as i64 as f64), F::Date(d) => *d as f64 / 1440.0, F::Str(s) => s.trim().parse().unwrap_or(0.0), F::Null => 0.0 }
    }
    pub fn truth(&self) -> bool {
        match self { F::Bool(b) => *b, F::Str(s) => matches!(s.to_lowercase().as_str(), "yes" | "true"), F::Null => false, x => x.num() != 0.0 }
    }
    fn text(&self) -> String { self.to_string() }
}

#[derive(Clone, Debug)]
pub enum Ex { Lit(F), Field(String), Neg(Box<Ex>), Not(Box<Ex>), Bin(String, Box<Ex>, Box<Ex>), Call(String, Vec<Ex>) }

/// What a formula can read: fields of the current row and working-time arithmetic.
pub trait Ctx {
    fn field(&self, name: &str) -> Option<F>;
    fn now(&self) -> i64;
    fn work_between(&self, a: i64, b: i64) -> i64;
    fn work_add(&self, a: i64, m: i64) -> i64;
}

struct P<'a> { s: &'a [u8], i: usize }

impl P<'_> {
    fn ws(&mut self) { while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() { self.i += 1 } }
    fn peek_word(&mut self, w: &str) -> bool {
        self.ws();
        let e = self.i + w.len();
        e <= self.s.len() && self.s[self.i..e].eq_ignore_ascii_case(w.as_bytes()) && self.s.get(e).is_none_or(|c| !c.is_ascii_alphanumeric())
    }
    fn eat(&mut self, w: &str) -> bool {
        self.ws();
        if self.s[self.i..].starts_with(w.as_bytes()) { self.i += w.len(); true } else { false }
    }
    fn word(&mut self, w: &str) -> bool { if self.peek_word(w) { self.i += w.len(); true } else { false } }
    fn bin(&mut self, next: fn(&mut Self) -> Result<Ex, String>, ops: &[&str]) -> Result<Ex, String> {
        let mut l = next(self)?;
        'outer: loop {
            for op in ops {
                let hit = if op.chars().all(|c| c.is_alphabetic()) { self.word(op) } else { self.eat(op) };
                if hit {
                    l = Ex::Bin(op.to_lowercase(), Box::new(l), Box::new(next(self)?));
                    continue 'outer;
                }
            }
            return Ok(l);
        }
    }
    fn or(&mut self) -> Result<Ex, String> { self.bin(Self::and, &["Or"]) }
    fn and(&mut self) -> Result<Ex, String> { self.bin(Self::not, &["And"]) }
    fn not(&mut self) -> Result<Ex, String> { if self.word("Not") { Ok(Ex::Not(Box::new(self.not()?))) } else { self.cmp() } }
    fn cmp(&mut self) -> Result<Ex, String> { self.bin(Self::cat, &["<=", ">=", "<>", "=", "<", ">"]) }
    fn cat(&mut self) -> Result<Ex, String> { self.bin(Self::add, &["&"]) }
    fn add(&mut self) -> Result<Ex, String> { self.bin(Self::modu, &["+", "-"]) }
    fn modu(&mut self) -> Result<Ex, String> { self.bin(Self::idiv, &["Mod"]) }
    fn idiv(&mut self) -> Result<Ex, String> { self.bin(Self::mul, &["\\"]) }
    fn mul(&mut self) -> Result<Ex, String> { self.bin(Self::neg, &["*", "/"]) }
    fn neg(&mut self) -> Result<Ex, String> { if self.eat("-") { Ok(Ex::Neg(Box::new(self.neg()?))) } else { self.eat("+"); self.pow() } }
    fn pow(&mut self) -> Result<Ex, String> {
        let b = self.atom()?;
        if self.eat("^") { Ok(Ex::Bin("^".into(), Box::new(b), Box::new(self.neg()?))) } else { Ok(b) }
    }
    fn atom(&mut self) -> Result<Ex, String> {
        self.ws();
        let Some(&c) = self.s.get(self.i) else { return Err("the formula ends too early".into()) };
        let rest = |p: &Self, end: u8| p.s[p.i + 1..].iter().position(|&x| x == end).map(|k| p.i + 1 + k);
        match c {
            b'(' => { self.i += 1; let e = self.or()?; if !self.eat(")") { return Err("a ) is missing".into()) } Ok(e) }
            b'[' => {
                let e = rest(self, b']').ok_or("a ] is missing")?;
                let name = String::from_utf8_lossy(&self.s[self.i + 1..e]).trim().to_string();
                self.i = e + 1;
                Ok(Ex::Field(name))
            }
            b'"' => {
                let mut out = String::new();
                self.i += 1;
                loop {
                    let e = rest(self, b'"').map(|k| k - 1).ok_or("a closing quote is missing")?;
                    out.push_str(&String::from_utf8_lossy(&self.s[self.i..e + 1]));
                    self.i = e + 2;
                    if self.s.get(self.i) == Some(&b'"') { out.push('"'); self.i += 1 } else { break }
                }
                Ok(Ex::Lit(F::Str(out)))
            }
            b'#' => {
                let e = rest(self, b'#').ok_or("a closing # is missing")?;
                let t = String::from_utf8_lossy(&self.s[self.i + 1..e]).to_string();
                self.i = e + 1;
                let d = crate::parse::date_in(&t, "%m/%d/%Y").map_err(|_| format!("#{t}# is not a date"))?;
                Ok(Ex::Lit(F::Date(crate::cal::at(d))))
            }
            b'0'..=b'9' | b'.' => {
                let st = self.i;
                while self.i < self.s.len() && (self.s[self.i].is_ascii_digit() || self.s[self.i] == b'.') { self.i += 1 }
                Ok(Ex::Lit(F::Num(String::from_utf8_lossy(&self.s[st..self.i]).parse().map_err(|_| "bad number")?)))
            }
            c if c.is_ascii_alphabetic() => {
                let st = self.i;
                while self.i < self.s.len() && (self.s[self.i].is_ascii_alphanumeric() || self.s[self.i] == b'_') { self.i += 1 }
                let name = String::from_utf8_lossy(&self.s[st..self.i]).to_lowercase();
                match name.as_str() {
                    "true" | "yes" => return Ok(Ex::Lit(F::Bool(true))),
                    "false" | "no" => return Ok(Ex::Lit(F::Bool(false))),
                    "null" => return Ok(Ex::Lit(F::Null)),
                    _ => {}
                }
                let mut args = vec![];
                if self.eat("(") && !self.eat(")") {
                    loop {
                        args.push(self.or()?);
                        if self.eat(")") { break }
                        if !self.eat(",") { return Err(format!("{name}( needs commas between values and a closing )")) }
                    }
                }
                if !FUNCS.contains(&name.as_str()) {
                    return Err(format!("{name} is not a known function"));
                }
                Ok(Ex::Call(name, args))
            }
            c => Err(format!("unexpected \"{}\"", c as char)),
        }
    }
}

pub const FUNCS: [&str; 34] = [
    "iif", "switch", "choose", "abs", "round", "int", "fix", "sgn", "sqr", "len", "left", "right", "mid", "instr", "ucase", "lcase", "trim",
    "isnull", "nz", "now", "date", "projdatediff", "projdateadd", "projdatesub", "datediff", "dateadd", "year", "month", "day", "weekday",
    "cstr", "cdbl", "max", "min",
];

pub fn parse(src: &str) -> Result<Ex, String> {
    let mut p = P { s: src.as_bytes(), i: 0 };
    let e = p.or()?;
    p.ws();
    if p.i < p.s.len() {
        return Err(format!("unexpected text at \"{}\"", String::from_utf8_lossy(&p.s[p.i..]).chars().take(12).collect::<String>()));
    }
    Ok(e)
}

/// Every field a formula names, for checking and for recalculation order.
pub fn fields(e: &Ex, out: &mut Vec<String>) {
    match e {
        Ex::Field(n) => out.push(n.clone()),
        Ex::Neg(a) | Ex::Not(a) => fields(a, out),
        Ex::Bin(_, a, b) => { fields(a, out); fields(b, out) }
        Ex::Call(_, v) => v.iter().for_each(|a| fields(a, out)),
        Ex::Lit(_) => {}
    }
}

pub fn eval(e: &Ex, c: &dyn Ctx) -> Result<F, String> {
    Ok(match e {
        Ex::Lit(v) => v.clone(),
        Ex::Field(n) => c.field(n).ok_or_else(|| format!("[{n}] is not a field"))?,
        Ex::Neg(a) => F::Num(-eval(a, c)?.num()),
        Ex::Not(a) => F::Bool(!eval(a, c)?.truth()),
        Ex::Bin(op, a, b) => {
            let (x, y) = (eval(a, c)?, eval(b, c)?);
            let date_math = matches!(x, F::Date(_)) || matches!(y, F::Date(_));
            match op.as_str() {
                "or" => F::Bool(x.truth() || y.truth()),
                "and" => F::Bool(x.truth() && y.truth()),
                "&" => F::Str(x.text() + &y.text()),
                "+" if matches!((&x, &y), (F::Str(_), F::Str(_))) => F::Str(x.text() + &y.text()),
                "+" if date_math => match (&x, &y) { (F::Date(d), o) | (o, F::Date(d)) => F::Date(d + (o.num() * 1440.0) as i64), _ => unreachable!() },
                "-" if matches!((&x, &y), (F::Date(_), F::Date(_))) => F::Num((x.num() - y.num()).round()),
                "-" if matches!(x, F::Date(_)) => match x { F::Date(d) => F::Date(d - (y.num() * 1440.0) as i64), _ => unreachable!() },
                "+" => F::Num(x.num() + y.num()),
                "-" => F::Num(x.num() - y.num()),
                "*" => F::Num(x.num() * y.num()),
                "/" => { if y.num() == 0.0 { return Err("division by zero".into()) } F::Num(x.num() / y.num()) }
                "\\" => { if y.num().trunc() == 0.0 { return Err("division by zero".into()) } F::Num((x.num().trunc() / y.num().trunc()).trunc()) }
                "mod" => { if y.num().trunc() == 0.0 { return Err("division by zero".into()) } F::Num(x.num().trunc() % y.num().trunc()) }
                "^" => F::Num(x.num().powf(y.num())),
                cmp => {
                    let o = match (&x, &y) {
                        (F::Str(a), F::Str(b)) => a.to_lowercase().cmp(&b.to_lowercase()),
                        (F::Str(a), b) | (b, F::Str(a)) if a.parse::<f64>().is_err() => {
                            let o = a.to_lowercase().cmp(&b.text().to_lowercase());
                            if matches!(x, F::Str(_)) { o } else { o.reverse() }
                        }
                        _ => x.num().partial_cmp(&y.num()).unwrap_or(std::cmp::Ordering::Equal),
                    };
                    use std::cmp::Ordering::*;
                    F::Bool(match cmp { "=" => o == Equal, "<>" => o != Equal, "<" => o == Less, ">" => o == Greater, "<=" => o != Greater, _ => o != Less })
                }
            }
        }
        Ex::Call(name, a) => {
            let arg = |k: usize| -> Result<F, String> { a.get(k).map(|x| eval(x, c)).unwrap_or(Ok(F::Null)) };
            let need = |n: usize| -> Result<(), String> { if a.len() < n { Err(format!("{name} needs {n} values")) } else { Ok(()) } };
            let s = |k: usize| -> Result<String, String> { Ok(arg(k)?.text()) };
            let n = |k: usize| -> Result<f64, String> { Ok(arg(k)?.num()) };
            let date = |k: usize| -> Result<i64, String> { Ok(match arg(k)? { F::Date(d) => d, F::Null => return Err("a date is empty".into()), x => (x.num() * 1440.0) as i64 }) };
            let chars = |t: &str| t.chars().collect::<Vec<_>>();
            match name.as_str() {
                "iif" => { need(3)?; if arg(0)?.truth() { arg(1)? } else { arg(2)? } }
                "switch" => {
                    let mut out = F::Null;
                    for pair in a.chunks(2) {
                        if pair.len() == 2 && eval(&pair[0], c)?.truth() { out = eval(&pair[1], c)?; break }
                    }
                    out
                }
                "choose" => { need(2)?; let k = n(0)? as usize; if k >= 1 && k < a.len() { arg(k)? } else { F::Null } }
                "abs" => F::Num(n(0)?.abs()),
                "round" => { let p = 10f64.powi(n(1)? as i32); F::Num((n(0)? * p).round() / p) }
                "int" => F::Num(n(0)?.floor()),
                "fix" => F::Num(n(0)?.trunc()),
                "sgn" => F::Num(n(0)?.signum()),
                "sqr" => F::Num(n(0)?.max(0.0).sqrt()),
                "len" => F::Num(s(0)?.chars().count() as f64),
                "left" => F::Str(chars(&s(0)?).into_iter().take(n(1)? as usize).collect()),
                "right" => { let v = chars(&s(0)?); let k = (n(1)? as usize).min(v.len()); F::Str(v[v.len() - k..].iter().collect()) }
                "mid" => {
                    let v = chars(&s(0)?);
                    let st = (n(1)? as usize).saturating_sub(1).min(v.len());
                    let len = if a.len() > 2 { n(2)? as usize } else { v.len() };
                    F::Str(v[st..].iter().take(len).collect())
                }
                "instr" => F::Num(s(0)?.to_lowercase().find(&s(1)?.to_lowercase()).map_or(0.0, |k| k as f64 + 1.0)),
                "ucase" => F::Str(s(0)?.to_uppercase()),
                "lcase" => F::Str(s(0)?.to_lowercase()),
                "trim" => F::Str(s(0)?.trim().into()),
                "isnull" => F::Bool(matches!(arg(0)?, F::Null) || matches!(arg(0)?, F::Str(ref x) if x.is_empty())),
                "nz" => match arg(0)? { F::Null => if a.len() > 1 { arg(1)? } else { F::Num(0.0) }, x => x },
                "now" | "date" => F::Date(c.now()),
                "projdatediff" => F::Num(c.work_between(date(0)?, date(1)?) as f64),
                "projdateadd" => F::Date(c.work_add(date(0)?, n(1)? as i64)),
                "projdatesub" => F::Date(c.work_add(date(0)?, -(n(1)? as i64))),
                "datediff" => {
                    let (u, x, y) = (s(0)?.to_lowercase(), date(1)?, date(2)?);
                    let mins = (y - x) as f64;
                    F::Num((mins / match u.as_str() { "n" => 1.0, "h" => 60.0, "ww" | "w" => 10080.0, "m" => 43830.0, "yyyy" => 525960.0, _ => 1440.0 }).trunc())
                }
                "dateadd" => {
                    let (u, k, d) = (s(0)?.to_lowercase(), n(1)?, date(2)?);
                    F::Date(d + (k * match u.as_str() { "n" => 1.0, "h" => 60.0, "ww" | "w" => 10080.0, "m" => 43830.0, "yyyy" => 525960.0, _ => 1440.0 }) as i64)
                }
                "year" | "month" | "day" | "weekday" => {
                    use chrono::Datelike;
                    let d = crate::cal::date_of(date(0)?);
                    F::Num(match name.as_str() { "year" => d.year() as f64, "month" => d.month() as f64, "day" => d.day() as f64, _ => d.weekday().number_from_sunday() as f64 })
                }
                "cstr" => F::Str(s(0)?),
                "cdbl" => F::Num(n(0)?),
                "max" => F::Num(a.iter().map(|x| eval(x, c).map(|v| v.num())).collect::<Result<Vec<_>, _>>()?.into_iter().fold(f64::MIN, f64::max)),
                "min" => F::Num(a.iter().map(|x| eval(x, c).map(|v| v.num())).collect::<Result<Vec<_>, _>>()?.into_iter().fold(f64::MAX, f64::min)),
                f => return Err(format!("{f} is not a known function")),
            }
        }
    })
}
