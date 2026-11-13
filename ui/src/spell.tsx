// Spelling (Home › Proofing, F7): the words of task names, notes, text custom fields and sheet text
// that the dictionary does not know, one at a time, with Change, Change All, Ignore, Ignore All and
// Add to Dictionary. The engine finds them (crates/core/src/spell.rs); the language and the user's
// own words are kept with the options on this computer.
import { For, Show, createSignal } from "solid-js";
import { act, call, openDialog, opts, setOpts, P } from "./store";
import { Dialog, Pick } from "./ui";
import { selectUid } from "./cmds";
import { Options } from "./api";

type Issue = { uid: number; sheet: number | null; field: string; place: string; text: string; start: number; word: string; suggestions: string[] };

/** The edit that writes `text` back where an issue was found. */
const writeOp = (x: Issue, text: string) => (x.sheet !== null ? { SheetCells: { k: x.sheet, cells: [[x.field, text]], fmt: null } } : { Set: { uid: x.uid, field: x.field, value: text } });
const spot = (x: Issue) => `${x.uid}|${x.sheet}|${x.field}`;

export const spelling = () => {
  const [lang, setLang] = createSignal(opts().spellLang || "en-GB");
  const [list, setList] = createSignal<Issue[]>([]);
  const [k, setK] = createSignal(0);
  const [to, setTo] = createSignal("");
  const [busy, setBusy] = createSignal(true);
  /** Words ignored in this check (Ignore All), and single places passed over (Ignore). */
  const ignored = new Set<string>(), passed = new Set<string>();
  const cur = () => list()[k()];
  const show = (i: number) => {
    const all = list();
    let j = i;
    while (j < all.length && passed.has(`${spot(all[j])}|${all[j].start}|${all[j].word}`)) j++;
    setK(j); const x = all[j];
    if (x) { setTo(x.suggestions[0] ?? x.word); if (x.sheet === null) selectUid(x.uid); }
  };
  /** Checks the plan again (after a change), carrying on from the same place in the list. */
  const run = async (from = 0) => {
    setBusy(true);
    const own = [...(opts().dictionary ?? []), ...ignored];
    const v = (await call<Issue[]>("spell_check", { lang: lang(), own })) ?? [];
    setList(v); setBusy(false); show(Math.min(from, v.length));
  };
  const next = () => show(k() + 1);
  const change = async (all: boolean) => {
    const x = cur(); if (!x) return;
    const word = to().trim();
    if (!word) return;
    // Every place of the word (Change All) or this one: each text is rewritten from its end, so the earlier places stay put.
    const targets = all ? list().filter(y => y.word === x.word) : [x];
    const byText = new Map<string, Issue[]>();
    targets.forEach(y => byText.set(spot(y), [...(byText.get(spot(y)) ?? []), y]));
    const ops = [...byText.values()].map(ys => {
      let t = ys[0].text;
      [...ys].sort((a, b) => b.start - a.start).forEach(y => { t = t.slice(0, y.start) + word + t.slice(y.start + y.word.length); });
      return writeOp(ys[0], t);
    });
    if (!(await act(ops.length === 1 ? ops[0] : { Batch: ops }))) return;
    await run(k());
  };
  const ignoreAll = () => { const x = cur(); if (!x) return; ignored.add(x.word); setList(list().filter(y => y.word !== x.word)); show(k()); };
  const ignoreOnce = () => { const x = cur(); if (!x) return; passed.add(`${spot(x)}|${x.start}|${x.word}`); next(); };
  const addWord = async () => {
    const x = cur(); if (!x) return;
    const o: Options = { ...opts(), dictionary: [...new Set([...(opts().dictionary ?? []), x.word])] };
    if (await call("set_options", { opts: o })) { setOpts(o); setList(list().filter(y => y.word.toLowerCase() !== x.word.toLowerCase())); show(k()); }
  };
  const setLanguage = async (l: string) => {
    setLang(l);
    const o: Options = { ...opts(), spellLang: l };
    if (await call("set_options", { opts: o })) setOpts(o);
    passed.clear(); run(0);
  };
  run(0);
  openDialog(() => (
    <Dialog title="Spelling" help="spelling" cancel="Close" extra={<Pick value={lang()} set={setLanguage} options={[["en-GB", "English (UK)"], ["en-US", "English (US)"]]} />}>
      <div class="spell">
        <Show when={!busy()} fallback={<p class="muted">Checking the plan…</p>}>
          <Show when={cur()} fallback={<p><b>The spelling check is complete.</b> {list().length ? "" : `No unknown words in ${P().name}.`}</p>}>{x => (
            <>
              <p class="muted">{x().place} <span class="grow" /> {k() + 1} of {list().length}</p>
              <label>Not in the dictionary:</label>
              <div class="spell-text">{x().text.slice(0, x().start)}<mark>{x().word}</mark>{x().text.slice(x().start + x().word.length)}</div>
              <label>Change to:</label>
              <input class="spell-to" value={to()} onInput={e => setTo(e.currentTarget.value)} onKeyDown={e => { if (e.key === "Enter") { e.preventDefault(); e.stopPropagation(); change(false); } }} />
              <div class="spell-row">
                <div class="spell-sugg">
                  <For each={x().suggestions} fallback={<span class="muted">(no suggestions)</span>}>{s => <button classList={{ on: s === to() }} onClick={() => setTo(s)} onDblClick={() => { setTo(s); change(false); }}>{s}</button>}</For>
                </div>
                <div class="spell-btns">
                  <button class="btn" onClick={ignoreOnce} title="Leave this one as it is">Ignore</button>
                  <button class="btn" onClick={ignoreAll} title="Leave this word everywhere in this check">Ignore All</button>
                  <button class="btn" onClick={addWord} title="A word of your own (a name, a term): never marked again on this computer">Add to Dictionary</button>
                  <button class="btn pri" onClick={() => change(false)}>Change</button>
                  <button class="btn" onClick={() => change(true)} title="Change this word everywhere in the plan">Change All</button>
                </div>
              </div>
            </>
          )}</Show>
        </Show>
      </div>
    </Dialog>
  ));
};
