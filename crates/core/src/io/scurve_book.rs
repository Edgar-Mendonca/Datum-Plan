//! An S-curve as a spreadsheet: per period the planned and actual progress, and the cumulative
//! planned, actual and forecast %, with a native chart (period bars on the left axis, cumulative
//! curves on the right) that the spreadsheet program draws and edits itself. Also the same table
//! as a sheet of the plan, with the period figures as formulas over the cumulative ones.
use crate::model::SheetData;
use crate::report::SCurve;
use chrono::{Datelike, NaiveDate};
use serde::Deserialize;
use std::path::Path;

/// The look chosen in the window's S-curve Format (title and colours as "#RRGGBB").
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct ScStyle { pub title: String, pub cp: String, pub ca: String, pub cf: String, pub bp: String, pub ba: String, pub pbars: bool, pub abars: bool, pub fc: String, pub smooth: bool, pub legend: String }
impl Default for ScStyle {
    fn default() -> Self {
        Self { title: String::new(), cp: "#1F3864".into(), ca: "#2E8B3E".into(), cf: "#E07B24".into(), bp: "#9DC3E6".into(), ba: "#A9D18E".into(), pbars: true, abars: true, fc: "auto".into(), smooth: true, legend: "bottom".into() }
    }
}

/// One period of the curve: its last day, and the cumulative planned, actual and forecast % at its end
/// (actual up to the period that holds the status date, measured on it; forecast from the status date on).
#[derive(Clone, Debug, PartialEq)]
pub struct Row { pub day: i64, pub planned: f64, pub actual: Option<f64>, pub forecast: Option<f64> }

/// The periods of a curve, without its extra status-date point. The forecast shows from the status period
/// on when `fc` is "on", or "auto" and it leaves the planned curve somewhere.
pub fn rows(c: &SCurve, fc: &str) -> Vec<Row> {
    let k = c.status_at.unwrap_or_else(|| c.days.iter().position(|&d| d >= c.status_day).unwrap_or(c.days.len()));
    let show = fc == "on" || (fc != "off" && (k..c.days.len()).any(|i| (c.forecast[i] - c.planned[i]).abs() > 0.05));
    (0..c.days.len()).filter(|&i| Some(i) != c.status_at).map(|i| Row {
        day: c.days[i], planned: c.planned[i],
        // The period holding the status date carries the actual measured on the status date.
        actual: match c.status_at { Some(s) if s + 1 == i => c.actual[s], _ => c.actual[i] },
        forecast: (show && i >= k).then_some(c.forecast[i]),
    }).collect()
}

fn date(d: i64) -> NaiveDate { NaiveDate::from_ymd_opt(1970, 1, 1).unwrap_or_default() + chrono::Duration::days(d) }
fn serial(d: i64) -> f64 { d as f64 + 25569.0 }
fn rgb(s: &str) -> u32 { u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap_or(0x1F3864) }
/// The status date and the planned and actual % on it, for under the title.
pub fn subtitle(c: &SCurve) -> String {
    let i = c.status_at.unwrap_or_else(|| c.days.iter().position(|&d| d >= c.status_day).unwrap_or(c.days.len().saturating_sub(1)).min(c.days.len().saturating_sub(1)));
    let a = c.actual.get(i).copied().flatten().unwrap_or(0.0);
    format!("Data date: {}   |   Planned {:.1}%   |   Actual {:.1}%   |   Weighted by {}", date(c.status_day).format("%d-%b-%y"), c.planned.get(i).copied().unwrap_or(0.0), a, c.weight)
}

const HEAD: [&str; 6] = ["Period ending", "Planned in period", "Actual in period", "Planned cumulative", "Actual cumulative", "Forecast cumulative"];
const FIRST: u32 = 5; // 0-based row of the first period (headings on row 4)

/// Writes the workbook: one sheet per curve (e.g. Weekly and Monthly), each with its table and chart.
pub fn write(path: &Path, curves: &[(String, SCurve, String)], st: &ScStyle) -> Result<(), String> {
    use rust_xlsxwriter::{Chart, ChartEmptyCells, ChartFont, ChartFormat, ChartLegendPosition, ChartLine, ChartLineDashType, ChartSolidFill, ChartType, Format, FormatAlign, FormatBorder, Formula, Workbook};
    let e = |x: rust_xlsxwriter::XlsxError| x.to_string();
    let mut wb = Workbook::new();
    let title_f = Format::new().set_bold().set_font_size(14).set_font_color(0x1F3864);
    let sub_f = Format::new().set_font_color(0x595959);
    let head_f = Format::new().set_bold().set_font_color(0xFFFFFF).set_background_color(0x1F3864).set_text_wrap().set_align(FormatAlign::Center).set_align(FormatAlign::VerticalCenter).set_border(FormatBorder::Thin).set_border_color(0xBFBFBF);
    let thin = |f: Format| f.set_border(FormatBorder::Thin).set_border_color(0xD9D9D9);
    let pct = thin(Format::new().set_num_format("0.0%"));
    let day_f = thin(Format::new().set_num_format("dd-mmm-yy").set_align(FormatAlign::Center));
    for (name, c, unit) in curves {
        let ws = wb.add_worksheet();
        ws.set_name(name).map_err(e)?;
        let title = if st.title.is_empty() { format!("{} – {} S-Curve", c.name, name) } else { format!("{} ({})", st.title, name.to_lowercase()) };
        ws.write_string_with_format(0, 0, &title, &title_f).map_err(e)?;
        ws.write_string_with_format(1, 0, subtitle(c), &sub_f).map_err(e)?;
        ws.write_string_with_format(2, 0, "Period figures are formulas over the cumulative columns: change a cumulative value and the bars and curves follow.", &Format::new().set_italic().set_font_color(0x8C8C8C)).map_err(e)?;
        ws.set_row_height(FIRST - 1, 30).map_err(e)?;
        for (j, h) in HEAD.iter().enumerate() { ws.write_string_with_format(FIRST - 1, j as u16, *h, &head_f).map_err(e)?; }
        ws.set_column_width(0, 13).map_err(e)?;
        for j in 1..6 { ws.set_column_width(j, 12.5).map_err(e)?; }
        let rs = rows(c, &st.fc);
        for (i, r) in rs.iter().enumerate() {
            let (row, x) = (FIRST + i as u32, FIRST + i as u32 + 1); // x: the 1-based row in formulas
            ws.write_number_with_format(row, 0, serial(r.day), &day_f).map_err(e)?;
            ws.write_number_with_format(row, 3, r.planned / 100.0, &pct).map_err(e)?;
            if let Some(a) = r.actual { ws.write_number_with_format(row, 4, a / 100.0, &pct).map_err(e)?; } else { ws.write_blank(row, 4, &pct).map_err(e)?; }
            if let Some(f) = r.forecast { ws.write_number_with_format(row, 5, f / 100.0, &pct).map_err(e)?; } else { ws.write_blank(row, 5, &pct).map_err(e)?; }
            let pp = if i == 0 { r.planned } else { r.planned - rs[i - 1].planned };
            let fp = if i == 0 { format!("=D{x}") } else { format!("=D{x}-D{}", x - 1) };
            ws.write_formula_with_format(row, 1, Formula::new(fp).set_result(format!("{}", pp / 100.0)), &pct).map_err(e)?;
            let ap = r.actual.map(|a| a - if i == 0 { 0.0 } else { rs[i - 1].actual.unwrap_or(0.0) });
            // Empty after the status date: no bar there, and the table stays clean.
            let fa = if i == 0 { format!("=IF(E{x}=\"\",\"\",E{x})") } else { format!("=IF(E{x}=\"\",\"\",E{x}-N(E{}))", x - 1) };
            let res = ap.map_or(" ".to_string(), |v| format!("{}", v / 100.0)); // a blank text result: previews show it empty
            ws.write_formula_with_format(row, 2, Formula::new(fa).set_result(res), &pct).map_err(e)?;
        }
        ws.set_freeze_panes(FIRST, 0).map_err(e)?;
        if rs.is_empty() { continue }
        let (a, b) = (FIRST, FIRST + rs.len() as u32 - 1);
        let cats = (name.as_str(), a, 0u16, b, 0u16);
        let bars = st.pbars || st.abars;
        let mut col = Chart::new(ChartType::Column);
        let mut lin = Chart::new(ChartType::Line);
        let mut font = ChartFont::new(); font.set_name("Calibri").set_size(9);
        let mut bold = font.clone(); bold.set_bold();
        let mut slant = font.clone(); slant.set_rotation(-45);
        let add_bar = |ch: &mut Chart, j: u16, color: &str| { ch.add_series().set_categories(cats).set_values((name.as_str(), a, j, b, j)).set_name((name.as_str(), FIRST - 1, j)).set_format(ChartFormat::new().set_solid_fill(ChartSolidFill::new().set_color(rgb(color)))).set_gap(55).set_overlap(-5); };
        if st.pbars { add_bar(&mut col, 1, &st.bp) }
        if st.abars { add_bar(&mut col, 2, &st.ba) }
        let any_fc = rs.iter().any(|r| r.forecast.is_some());
        for (j, color, dash) in [(3u16, &st.cp, false), (4, &st.ca, false), (5, &st.cf, true)] {
            if j == 5 && !any_fc { continue }
            let mut line = ChartLine::new();
            line.set_color(rgb(color)).set_width(2.25);
            if dash { line.set_dash_type(ChartLineDashType::Dash); }
            let s = lin.add_series();
            s.set_categories(cats).set_values((name.as_str(), a, j, b, j)).set_name((name.as_str(), FIRST - 1, j)).set_format(ChartFormat::new().set_line(&line)).set_smooth(st.smooth).set_secondary_axis(bars);
        }
        lin.show_empty_cells_as(ChartEmptyCells::Gaps);
        let ch = if bars { col.combine(&lin); &mut col } else { &mut lin };
        ch.show_empty_cells_as(ChartEmptyCells::Gaps);
        let mut tf = ChartFont::new(); tf.set_name("Calibri").set_size(14).set_bold().set_color(0x1F3864);
        ch.title().set_name(&title).set_font(&tf);
        ch.legend().set_position(if st.legend == "top" { ChartLegendPosition::Top } else { ChartLegendPosition::Bottom }).set_font(&font);
        let rotate = rs.len() > 10;
        ch.x_axis().set_num_format(if unit == "month" { "mmm-yy" } else { "dd-mmm-yy" }).set_font(if rotate { &slant } else { &font }).set_text_axis(true);
        let per = if unit == "month" { "Month %" } else if unit == "week" { "Week %" } else { "Period %" };
        if bars {
            ch.y_axis().set_name(per).set_num_format("0.0%").set_font(&font).set_name_font(&bold).set_min(0);
            ch.y2_axis().set_name("Cumulative %").set_num_format("0%").set_min(0).set_max(1).set_font(&font).set_name_font(&bold);
        } else {
            ch.y_axis().set_name("Cumulative %").set_num_format("0%").set_min(0).set_max(1).set_font(&font).set_name_font(&bold);
        }
        ch.set_width(980).set_height(500);
        ws.insert_chart(FIRST - 1, 7, ch).map_err(e)?;
    }
    wb.save(path).map_err(e)
}

/// The same table as a sheet of the plan: dates and cumulative values typed, the period figures as
/// formulas, so the sheet can be checked, edited, used in other formulas and charted on a dashboard.
pub fn sheet(name: &str, c: &SCurve, st: &ScStyle) -> SheetData {
    let mut s = SheetData { name: name.into(), ..Default::default() };
    let cols = ["A", "B", "C", "D", "E", "F"];
    let title = if st.title.is_empty() { format!("{} – S-Curve", c.name) } else { st.title.clone() };
    s.cells.insert("A1".into(), title);
    s.style.insert("A1".into(), "b;z:18;c:#1F3864".into());
    s.cells.insert("A2".into(), subtitle(c));
    s.style.insert("A2".into(), "c:#595959".into());
    for (j, h) in HEAD.iter().enumerate() {
        let a = format!("{}4", cols[j]);
        s.cells.insert(a.clone(), h.to_string());
        s.style.insert(a, "b;w;a:c;c:#FFFFFF;f:#1F3864".into());
    }
    for (j, w) in [96u16, 100, 100, 110, 110, 110].iter().enumerate() { s.widths.insert(j as u32, *w); }
    let rs = rows(c, &st.fc);
    for (i, r) in rs.iter().enumerate() {
        let x = i + 5;
        let d = date(r.day);
        s.cells.insert(format!("A{x}"), format!("=DATE({},{},{})", d.year(), d.month(), d.day()));
        s.fmt.insert(format!("A{x}"), "dd-mmm-yy".into());
        s.cells.insert(format!("D{x}"), format!("{}", (r.planned * 10.0).round() / 1000.0));
        if let Some(a) = r.actual { s.cells.insert(format!("E{x}"), format!("{}", (a * 10.0).round() / 1000.0)); }
        if let Some(f) = r.forecast { s.cells.insert(format!("F{x}"), format!("{}", (f * 10.0).round() / 1000.0)); }
        s.cells.insert(format!("B{x}"), if i == 0 { format!("=D{x}") } else { format!("=D{x}-D{}", x - 1) });
        s.cells.insert(format!("C{x}"), if i == 0 { format!("=IF(E{x}=\"\",\"\",E{x})") } else { format!("=IF(E{x}=\"\",\"\",E{x}-N(E{}))", x - 1) });
        for col in ["B", "C", "D", "E", "F"] { s.fmt.insert(format!("{col}{x}"), "0.0%".into()); }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    fn curve() -> SCurve {
        // Three periods with the status date between the second and the third (an extra point).
        SCurve { days: vec![100, 107, 110, 114, 121], planned: vec![0.0, 20.0, 30.0, 50.0, 100.0], actual: vec![Some(0.0), Some(15.0), Some(22.0), None, None],
                 forecast: vec![0.0, 15.0, 22.0, 45.0, 100.0], total: 1.0, status_day: 110, name: "Plan".into(), weight: "duration".into(), status_at: Some(2) }
    }
    #[test]
    fn rows_leave_out_the_status_point_and_carry_its_values() {
        let r = rows(&curve(), "auto");
        assert_eq!(r.iter().map(|x| x.day).collect::<Vec<_>>(), vec![100, 107, 114, 121]);
        // The period holding the status date (ends 114) shows the actual measured on the status date.
        assert_eq!(r[2].actual, Some(22.0));
        assert_eq!(r[3].actual, None);
        // The forecast differs from the plan, so it shows from the status period on.
        assert_eq!(r.iter().map(|x| x.forecast).collect::<Vec<_>>(), vec![None, None, Some(45.0), Some(100.0)]);
        assert!(rows(&curve(), "off").iter().all(|x| x.forecast.is_none()));
    }
    #[test]
    fn workbook_has_a_native_chart_and_formulas() {
        let dir = std::env::temp_dir().join(format!("dp_sc_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("s.xlsx");
        write(&p, &[("Weekly".into(), curve(), "week".into()), ("Monthly".into(), curve(), "month".into())], &ScStyle::default()).unwrap();
        let z = std::fs::read(&p).unwrap();
        let text = String::from_utf8_lossy(&z);
        assert!(text.contains("xl/charts/chart1.xml") && text.contains("xl/charts/chart2.xml"), "two native charts");
        std::fs::remove_dir_all(&dir).ok();
    }
    #[test]
    fn sheet_has_dates_values_and_period_formulas() {
        let s = sheet("S-Curve (Weekly)", &curve(), &ScStyle::default());
        assert_eq!(s.cells.get("A5").map(String::as_str), Some("=DATE(1970,4,11)"));
        assert_eq!(s.cells.get("D6").map(String::as_str), Some("0.2"));
        assert_eq!(s.cells.get("B6").map(String::as_str), Some("=D6-D5"));
        assert_eq!(s.cells.get("E7").map(String::as_str), Some("0.22"));
        assert_eq!(s.fmt.get("C6").map(String::as_str), Some("0.0%"));
    }
    #[test]
    fn the_sheet_calculates_in_the_plan() {
        let mut p = crate::templates::build(&crate::templates::TEMPLATES[0], NaiveDate::from_ymd_opt(2026, 10, 26).unwrap());
        p.sheets.push(sheet("S-Curve Weekly", &curve(), &ScStyle::default()));
        let c = crate::fields::Calc::new(&p, "%d-%b-%y", NaiveDate::from_ymd_opt(2026, 11, 10).unwrap());
        let v = crate::sheet::sheet_view(&c, p.sheets.len() - 1).unwrap();
        let shown = |a: &str| v.cells.iter().find(|x| x.0 == a).map(|x| x.2.clone()).unwrap_or_default();
        assert_eq!(shown("B6"), "20.0%");
        assert_eq!(shown("C7"), "7.0%");
        assert_eq!(shown("C8"), "");
        assert_eq!(shown("A5"), "11-Apr-70");
    }
}
