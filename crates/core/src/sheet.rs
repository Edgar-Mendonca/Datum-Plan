//! Excel formulas through the Gridcraft engine: formula columns on tasks (`=[% Complete]-[Baseline
//! % Complete]`) and the plan's spreadsheet tabs, which can also read a "Plan" sheet of all tasks.
use crate::cal::{date_of, DAY};
use crate::fields::{money, Calc, Kind};
use crate::formula::F;
use crate::model::SheetData;
use gridcraft_core::{CellRef, Value};
use gridcraft_model::{Cell, Formula, Sheet, Workbook};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;

/// 1970-01-01 as an Excel date serial.
const EPOCH: f64 = 25569.0;

/// Our value as an Excel value: dates become serials, durations become days.
pub fn to_value(v: &F, kind: Kind, day_min: i64) -> Value {
    match v {
        F::Null => Value::Empty,
        F::Num(n) if kind == Kind::Dur => Value::number(n / day_min as f64),
        F::Num(n) => Value::number(*n),
        F::Str(s) => Value::text(s.as_str()),
        F::Bool(b) => Value::Bool(*b),
        F::Date(m) => Value::number(*m as f64 / DAY as f64 + EPOCH),
    }
}

/// An Excel result in the field's kind.
pub fn from_value(v: &Value, kind: Kind, day_min: i64) -> F {
    let num = || match v { Value::Number(n) => Some(*n), Value::Bool(b) => Some(*b as u8 as f64), _ => None };
    match (v, kind) {
        (Value::Empty, _) => F::Null,
        (Value::Error(_), _) => F::Str(v.display()),
        (_, Kind::Date | Kind::Fin) => num().map_or_else(|| F::Str(v.display()), |n| F::Date(((n - EPOCH) * DAY as f64).round() as i64)),
        (_, Kind::Dur) => num().map_or_else(|| F::Str(v.display()), |n| F::Num((n * day_min as f64).round())),
        (_, Kind::Flag) => F::Bool(v.to_bool().unwrap_or(false)),
        (_, Kind::Text) => F::Str(v.display()),
        _ => num().map_or_else(|| F::Str(v.display()), F::Num),
    }
}

/// Column letters for a 0-based index (0 → A, 26 → AA).
pub fn col_name(mut c: u32) -> String {
    let mut s = String::new();
    loop {
        s.insert(0, (b'A' + (c % 26) as u8) as char);
        if c < 26 {
            return s;
        }
        c = c / 26 - 1;
    }
}

/// Replaces `[Field]` references (outside string literals) using `col`, which returns the cell
/// reference to put in their place. Unknown fields are an error.
pub fn translate(src: &str, mut col: impl FnMut(&str) -> Option<String>) -> Result<String, String> {
    let body = src.trim().strip_prefix('=').unwrap_or(src.trim());
    let (mut out, mut quoted, mut chars) = (String::new(), false, body.chars().peekable());
    while let Some(ch) = chars.next() {
        match ch {
            '"' => { quoted = !quoted; out.push(ch) }
            '[' if !quoted => {
                let name: String = chars.by_ref().take_while(|&c| c != ']').collect();
                out.push_str(&col(name.trim()).ok_or_else(|| format!("There is no field called [{}].", name.trim()))?);
            }
            _ => out.push(ch),
        }
    }
    Ok(out)
}

/// Checks an Excel-style formula: its fields exist and it parses.
pub fn check(src: &str, known: impl Fn(&str) -> bool) -> Result<(), String> {
    let t = translate(src, |n| known(n).then(|| "A1".into()))?;
    match Formula::new(&t).expr {
        Some(_) => Ok(()),
        None => Err("The formula is not valid. Check brackets, quotes and commas.".into()),
    }
}

pub fn is_excel(formula: &str) -> bool {
    formula.trim_start().starts_with('=')
}

/// Evaluates every Excel formula column for every task. Formula columns may read each other.
pub fn task_formulas(c: &Calc, defs: &[(String, String)]) -> HashMap<String, Vec<F>> {
    let p = c.p;
    let n = p.tasks.len();
    let mut wb = Workbook::new();
    // Formula columns first (A, B, …), then the plain fields they read.
    let mut cols: HashMap<String, u32> = defs.iter().enumerate().map(|(k, (key, _))| (format!("c:{key}"), k as u32)).collect();
    let mut texts = vec![];
    for (_, f) in defs {
        let t = translate(f, |name| {
            let key = c.key_of(name)?;
            let next = cols.len() as u32;
            let col = *cols.entry(key).or_insert(next);
            Some(format!("{}{{ROW}}", col_name(col)))
        });
        texts.push(t.ok());
    }
    let sheet: &mut Sheet = match wb.sheet_mut(0) { Some(s) => s, None => return HashMap::new() };
    for (key, &col) in cols.iter().filter(|(k, _)| !defs.iter().any(|(d, _)| &&format!("c:{d}") == k)) {
        let kind = c.kind(key);
        for i in 0..n {
            sheet.set_value(CellRef::new(i as u32, col), to_value(&c.val(i, key), kind, p.day_min));
        }
    }
    for (k, t) in texts.iter().enumerate() {
        for i in 0..n {
            let cell = match t {
                Some(t) => Cell::formula(Formula::new(&t.replace("{ROW}", &(i + 1).to_string()))),
                None => Cell::value(Value::Error(gridcraft_core::CellError::Name)),
            };
            sheet.set_cell(CellRef::new(i as u32, k as u32), cell);
        }
    }
    gridcraft_calc::Calc::new().recalc_all(&mut wb);
    let sheet = &wb.sheets[0];
    defs.iter().enumerate().map(|(k, (key, _))| {
        let kind = c.kind(&format!("c:{key}"));
        (key.clone(), (0..n).map(|i| from_value(&sheet.value(CellRef::new(i as u32, k as u32)), kind, p.day_min)).collect())
    }).collect()
}

/// Fields on the "Plan" sheet that spreadsheet tabs can read (row 1 holds the titles).
pub const PLAN_COLS: [&str; 18] = ["id", "code", "name", "level", "summary", "dur", "start", "finish", "pct", "b0start", "b0finish", "b0dur", "cost", "b0cost", "work", "tf", "critical", "res"];

#[derive(Serialize)]
pub struct SheetOut {
    pub name: String,
    pub rows: u32,
    pub cols: u32,
    /// Non-empty cells: address, what was typed, what is shown, and whether it is a number.
    pub cells: Vec<(String, String, String, bool)>,
}

fn show(v: &Value, fmt: &str, c: &Calc) -> (String, bool) {
    match (v, fmt) {
        (Value::Number(n), "date") => (date_of(((n - EPOCH) * DAY as f64) as i64).format(&c.fmt).to_string(), true),
        (Value::Number(n), "pct") => (format!("{:.1}%", n * 100.0), true),
        (Value::Number(n), "money") => (money(*n, &c.p.currency), true),
        (Value::Number(n), "0") => (format!("{n:.0}"), true),
        (Value::Number(n), "0.00") => (format!("{n:.2}"), true),
        (Value::Number(n), _) => {
            let s = format!("{:.6}", n);
            (s.trim_end_matches('0').trim_end_matches('.').to_string(), true)
        }
        (v, _) => (v.display(), false),
    }
}

/// The plan's workbook: the Plan sheet first, then the user's tabs, all calculated.
pub fn book(c: &Calc) -> Workbook {
    let p = c.p;
    let mut wb = Workbook::new();
    if let Some(sh) = wb.sheet_mut(0) {
        sh.name = "Plan".into();
        for (k, key) in PLAN_COLS.iter().enumerate() {
            sh.set_value(CellRef::new(0, k as u32), Value::text(c.name(key).as_str()));
            let kind = c.kind(key);
            for i in 0..p.tasks.len() {
                sh.set_value(CellRef::new(i as u32 + 1, k as u32), to_value(&c.val(i, key), kind, p.day_min));
            }
        }
    }
    for s in &p.sheets {
        let mut sh = Sheet::new(s.name.as_str());
        for (a1, input) in &s.cells {
            let Some(at) = CellRef::parse(a1) else { continue };
            let cell = if input.starts_with('=') && input.len() > 1 {
                Cell::formula(Formula::new(input))
            } else {
                Cell::value(gridcraft_core::parse::parse_input(input, wb.date_system).value)
            };
            sh.set_cell(at, cell);
        }
        wb.sheets.push(Arc::new(sh));
    }
    gridcraft_calc::Calc::new().recalc_all(&mut wb);
    wb
}

/// One spreadsheet tab as shown: `k` is its index in `Project.sheets` (the Plan sheet is not
/// editable and is shown through the task views).
pub fn sheet_view(c: &Calc, k: usize) -> Option<SheetOut> {
    let s: &SheetData = c.p.sheets.get(k)?;
    let wb = book(c);
    let sh = wb.sheet(k + 1)?;
    let mut cells = vec![];
    let (mut rows, mut cols) = (0, 0);
    for (a1, input) in &s.cells {
        let Some(at) = CellRef::parse(a1) else { continue };
        let (text, num) = show(&sh.value(at), s.fmt.get(a1).map_or("", String::as_str), c);
        rows = rows.max(at.row + 1);
        cols = cols.max(at.col + 1);
        cells.push((a1.clone(), input.clone(), text, num));
    }
    Some(SheetOut { name: s.name.clone(), rows, cols, cells })
}
