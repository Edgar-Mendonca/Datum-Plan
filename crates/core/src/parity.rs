//! Plans that carry their own dates (tests/parity): Datum Plan must give every task the same
//! start and finish, to the minute. See tests/parity/README.md.
use crate::{cal::date_of, io::{mspdi, stated_dates, xer}, sched::schedule, Project};

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
        let want = stated_dates(&text, name.ends_with(".xml"));
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
