//! Plans that carry their own dates (tests/parity): Datum Plan must give every task the same
//! start and finish, to the minute. See tests/parity/README.md.
use crate::{cal::date_of, io::{mspdi, xer, xml}, sched::schedule, Project};
use std::collections::HashMap;

/// Task uid → (start, finish) as the file states them, "YYYY-MM-DD HH:MM".
fn stated(name: &str, text: &str) -> HashMap<u32, (String, String)> {
    let cut = |s: &str| s.replace('T', " ").chars().take(16).collect::<String>();
    if name.ends_with(".xml") {
        let root = xml::parse(text).unwrap_or_else(|e| panic!("{name}: {e}"));
        root.get("Tasks").into_iter().flat_map(|t| t.all("Task"))
            .filter(|t| t.val("OutlineLevel") != "0" && !t.val("Start").is_empty())
            .filter_map(|t| Some((t.val("UID").parse().ok()?, (cut(t.val("Start")), cut(t.val("Finish"))))))
            .collect()
    } else {
        let (mut fields, mut table, mut out) = (vec![], String::new(), HashMap::new());
        for line in text.lines() {
            let v: Vec<&str> = line.split('\t').collect();
            match v[0] {
                "%T" => table = v.get(1).unwrap_or(&"").to_string(),
                "%F" => fields = v[1..].to_vec(),
                "%R" if table == "TASK" => {
                    let f = |k: &str| fields.iter().position(|x| *x == k).and_then(|i| v.get(i + 1)).map_or(String::new(), |s| cut(s));
                    if let Ok(uid) = f("task_id").parse() { out.insert(uid, (f("early_start_date"), f("early_end_date"))); }
                }
                _ => {}
            }
        }
        out
    }
}

#[test]
fn files_keep_their_dates() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/parity");
    let mut files: Vec<_> = std::fs::read_dir(&dir).map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).collect()).unwrap_or_default();
    files.retain(|f| matches!(f.extension().and_then(|e| e.to_str()), Some("xml" | "xer")));
    files.sort();
    let (mut checked, mut wrong) = (0, vec![]);
    for f in &files {
        let name = f.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        let text = std::fs::read_to_string(f).unwrap_or_default();
        let p: Project = if name.ends_with(".xml") { mspdi::read(&text) } else { xer::read(&text) }.unwrap_or_else(|e| panic!("{name}: {e}"));
        let s = schedule(&p).unwrap_or_else(|e| panic!("{name}: {e}"));
        let fmt = |t: i64| date_of(t).format("%Y-%m-%d").to_string() + &format!(" {:02}:{:02}", t.rem_euclid(1440) / 60, t.rem_euclid(60));
        let want = stated(&name, &text);
        for (i, t) in p.tasks.iter().enumerate() {
            let Some((ws, wf)) = want.get(&t.uid) else { continue };
            checked += 1;
            let (gs, gf) = (fmt(s.es[i]), fmt(s.ef[i]));
            if (&gs, &gf) != (ws, wf) { wrong.push(format!("{name} \"{}\": file {ws} → {wf}, Datum Plan {gs} → {gf}", t.name)) }
        }
    }
    assert!(checked >= 18, "only {checked} tasks were checked");
    assert!(wrong.is_empty(), "{} of {checked} tasks differ:\n{}", wrong.len(), wrong.join("\n"));
}
