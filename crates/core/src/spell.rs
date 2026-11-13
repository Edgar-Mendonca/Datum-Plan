//! Spelling: the words of task names, notes, text custom fields and sheet text that the English
//! dictionary (US or UK, from SCOWL; see dict/LICENSE-*.txt) does not know, with suggestions.
//! Acronyms (HAZOP, FAT), words with digits (PX-200B), mixed case (iPhone), web and e-mail
//! addresses and the user's own words are left alone.
use crate::model::Project;
use serde::Serialize;
use spellbook::Dictionary;
use std::collections::HashMap;
use std::sync::OnceLock;

static US: OnceLock<Option<Dictionary>> = OnceLock::new();
static GB: OnceLock<Option<Dictionary>> = OnceLock::new();

/// The dictionary for "en-GB" (British) or anything else (American), loaded once.
fn dict(lang: &str) -> Option<&'static Dictionary> {
    if lang.eq_ignore_ascii_case("en-GB") {
        GB.get_or_init(|| Dictionary::new(include_str!("../dict/en_GB.aff"), include_str!("../dict/en_GB.dic")).ok()).as_ref()
    } else {
        US.get_or_init(|| Dictionary::new(include_str!("../dict/en_US.aff"), include_str!("../dict/en_US.dic")).ok()).as_ref()
    }
}

/// One unknown word: where it is (a task's field or a sheet cell), the whole text, the word's place in
/// it (in UTF-16 units, as the window counts) and the dictionary's suggestions.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Issue {
    pub uid: u32,
    pub sheet: Option<usize>,
    pub field: String,
    pub place: String,
    pub text: String,
    pub start: usize,
    pub word: String,
    pub suggestions: Vec<String>,
}

/// The words of `text` to check, with their byte offsets.
pub fn words(text: &str) -> Vec<(usize, &str)> {
    let is_part = |c: char| c.is_alphanumeric() || c == '\'' || c == '’';
    let mut out = vec![];
    let mut push = |s: usize, e: usize| {
        let w = text[s..e].trim_matches(|c| c == '\'' || c == '’');
        if w.is_empty() { return }
        let s = s + text[s..e].find(w).unwrap_or(0);
        let letters = w.chars().filter(|c| c.is_alphabetic()).count();
        let upper_inside = w.chars().skip(1).any(char::is_uppercase);
        if letters >= 2 && !w.chars().any(|c| c.is_ascii_digit()) && !upper_inside && !in_address(text, s) { out.push((s, w)) }
    };
    let mut start: Option<usize> = None;
    for (i, c) in text.char_indices() {
        match (start, is_part(c)) {
            (None, true) => start = Some(i),
            (Some(s), false) => { push(s, i); start = None; }
            _ => {}
        }
    }
    if let Some(s) = start { push(s, text.len()) }
    out
}
/// Is the word at byte `at` part of a web or e-mail address?
fn in_address(text: &str, at: usize) -> bool {
    let a = text[..at].rfind(char::is_whitespace).map_or(0, |i| i + 1);
    let b = text[at..].find(char::is_whitespace).map_or(text.len(), |e| at + e);
    let chunk = &text[a..b];
    chunk.contains('@') || chunk.contains("://") || chunk.to_lowercase().starts_with("www.")
}
/// A cell address in reading order (row, then column).
fn a1_key(a: &str) -> (u32, u32) {
    let col = a.chars().take_while(char::is_ascii_alphabetic).fold(0u32, |n, c| n * 26 + (c.to_ascii_uppercase() as u32 - 64));
    (a.trim_start_matches(|c: char| c.is_ascii_alphabetic()).parse().unwrap_or(0), col)
}

/// Is `w` a known word (or one the user added)?
fn known(d: &Dictionary, w: &str, own: &[String]) -> bool {
    let w = w.replace('’', "'");
    own.iter().any(|o| o.eq_ignore_ascii_case(&w)) || d.check(&w)
}

/// Every unknown word in the plan, in order: tasks (name, notes, text custom fields), then sheets.
/// `own` holds the user's dictionary and the words ignored for now.
pub fn check(p: &Project, lang: &str, own: &[String]) -> Vec<Issue> {
    let Some(d) = dict(lang) else { return vec![] };
    let mut sugg: HashMap<String, Vec<String>> = HashMap::new();
    let mut out = vec![];
    let mut scan = |uid: u32, sheet: Option<usize>, field: &str, place: String, text: &str, out: &mut Vec<Issue>| {
        for (s, w) in words(text) {
            if known(d, w, own) { continue }
            // Suggestions are worked out once per word, for the first 500 different words (a long list stays quick).
            let n = sugg.len();
            let suggestions = sugg.entry(w.to_string()).or_insert_with(|| { let mut v = vec![]; if n < 500 { d.suggest(&w.replace('’', "'"), &mut v); } v.truncate(6); v }).clone();
            out.push(Issue { uid, sheet, field: field.into(), place: place.clone(), text: text.into(), start: text[..s].encode_utf16().count(), word: w.into(), suggestions });
        }
    };
    for (i, t) in p.tasks.iter().enumerate() {
        let id = i + 1;
        scan(t.uid, None, "name", format!("Task {id} · Name"), &t.name, &mut out);
        scan(t.uid, None, "notes", format!("Task {id} · Notes"), &t.notes, &mut out);
        for (k, v) in &t.custom {
            if let Some(s) = v.as_str() {
                let name = p.fields.iter().find(|f| &f.key == k).map_or(k.as_str(), |f| if f.alias.is_empty() { f.key.as_str() } else { f.alias.as_str() });
                scan(t.uid, None, &format!("c:{k}"), format!("Task {id} · {name}"), s, &mut out);
            }
        }
    }
    for (k, s) in p.sheets.iter().enumerate() {
        let mut cells: Vec<(&String, &String)> = s.cells.iter().filter(|(_, v)| !v.starts_with('=') && v.trim().parse::<f64>().is_err()).collect();
        cells.sort_by_key(|(a, _)| a1_key(a));
        for (a, v) in cells { scan(0, Some(k), a, format!("{} › {a}", s.name), v, &mut out) }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn words_skip_acronyms_codes_and_addresses() {
        let w: Vec<&str> = words("HAZOP close-out for PX-200B iPhone, see www.example.com or a@b.com; vendor's recieved").into_iter().map(|x| x.1).collect();
        assert_eq!(w, vec!["close", "out", "for", "see", "or", "vendor's", "recieved"]);
    }
    #[test]
    fn finds_unknown_words_with_suggestions_and_places() {
        let mut p = crate::templates::build(&crate::templates::TEMPLATES[0], chrono::NaiveDate::from_ymd_opt(2026, 11, 2).unwrap());
        p.tasks[1].name = "Recieve the compresor".into();
        p.sheets.push(crate::model::SheetData { name: "Vendors".into(), ..Default::default() });
        p.sheets[0].cells.insert("B2".into(), "Shiping docs".into());
        p.sheets[0].cells.insert("A1".into(), "=SUM(1)".into());
        let v = check(&p, "en-US", &[]);
        let words: Vec<&str> = v.iter().map(|x| x.word.as_str()).collect();
        assert!(words.contains(&"Recieve") && words.contains(&"compresor") && words.contains(&"Shiping"), "{words:?}");
        let r = v.iter().find(|x| x.word == "Recieve").unwrap();
        assert_eq!((r.field.as_str(), r.start, r.place.as_str()), ("name", 0, "Task 2 · Name"));
        assert!(r.suggestions.iter().any(|s| s == "Receive"), "{:?}", r.suggestions);
        assert_eq!(v.iter().find(|x| x.word == "Shiping").unwrap().place, "Vendors › B2");
        // Added words and the other English: "colour" is British.
        assert!(check(&p, "en-US", &["compresor".into(), "Recieve".into(), "Shiping".into()]).iter().all(|x| x.word != "compresor"));
        p.tasks[1].name = "Paint colour".into();
        assert!(check(&p, "en-GB", &[]).iter().all(|x| x.word != "colour"));
        assert!(check(&p, "en-US", &[]).iter().any(|x| x.word == "colour"));
    }
}
