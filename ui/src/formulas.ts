// Formula text helpers for the Sheets editor: where references are, what to suggest while typing,
// which argument the caret is in, and F4 ($ anchors). Pure functions over the typed text.

export const colName = (c: number) => { let s = ""; c += 1; while (c > 0) { const m = (c - 1) % 26; s = String.fromCharCode(65 + m) + s; c = Math.floor((c - 1) / 26); } return s; };
export const colIndex = (s: string) => { let c = 0; for (const ch of s.replace(/\$/g, "").toUpperCase()) c = c * 26 + ch.charCodeAt(0) - 64; return c - 1; };
export const a1 = (r: number, c: number) => `${colName(c)}${r + 1}`;

/** The text with string literals blanked out (same length), so their contents are never parsed. */
export const blankStrings = (t: string) => t.replace(/"(?:[^"]|"")*"?/g, m => '"' + " ".repeat(Math.max(0, m.length - 2)) + (m.length > 1 && m.endsWith('"') ? '"' : " "));

/** A sheet name as written before `!` (quoted when it has spaces or symbols). */
export const sheetRef = (name: string) => (/^[A-Za-z_][\w.]*$/.test(name) ? name : `'${name.replace(/'/g, "''")}'`);

export type Ref = { start: number; end: number; sheet: string; r0: number; c0: number; r1: number; c1: number };
const REF = /(?<![\w.\]$])(?:('(?:[^']|'')+'|[A-Za-z_][\w.]*)!)?(\$?[A-Z]{1,3}\$?\d{1,7})(?::(\$?[A-Z]{1,3}\$?\d{1,7}))?(?![\w(\[])/gi;
const cell = (s: string): [number, number] => { const m = s.replace(/\$/g, "").match(/^([A-Z]+)(\d+)$/i)!; return [+m[2] - 1, colIndex(m[1])]; };
/** The cell references in a formula, with their place in the text (for coloured boxes). */
export const refs = (t: string): Ref[] => {
  const out: Ref[] = [];
  for (const m of blankStrings(t).matchAll(REF)) {
    const [r0, c0] = cell(m[2]), [r1, c1] = m[3] ? cell(m[3]) : [r0, c0];
    out.push({ start: m.index!, end: m.index! + m[0].length, sheet: (m[1] ?? "").replace(/^'|'$/g, "").replace(/''/g, "'"), r0: Math.min(r0, r1), c0: Math.min(c0, c1), r1: Math.max(r0, r1), c1: Math.max(c0, c1) });
  }
  return out;
};

/** Whether a click on a cell should add a reference: the caret follows `=`, `(`, `,` or an operator. */
export const canPoint = (t: string, caret: number) => t.startsWith("=") && /[=(,+\-*/^&<>:;%\s]$/.test(blankStrings(t.slice(0, caret)));

export type Fn = [name: string, signature: string, about: string, category: string];
export type Catalog = { functions: Fn[]; names: [string, string][]; tasks: string[]; resources: string[] };
export type Suggestion = { label: string; insert: string; about: string; from: number };
/** What to offer while typing a formula: functions, the plan's names and tables, or a table's columns. */
export const suggest = (t: string, caret: number, cat: Catalog | undefined): Suggestion[] => {
  if (!cat || !t.startsWith("=")) return [];
  const before = blankStrings(t.slice(0, caret));
  const col = before.match(/(Tasks|Resources)\[([^\[\]]*)$/i);
  if (col) {
    const q = col[2].toLowerCase(), list = col[1].toLowerCase() === "tasks" ? cat.tasks : cat.resources;
    return list.filter(c => c.toLowerCase().includes(q)).slice(0, 12).map(c => ({ label: c, insert: `${c}]`, about: `${col[1]} column`, from: caret - col[2].length }));
  }
  const w = before.match(/(?:^=|[=(,+\-*/^&<>:;\s])([A-Za-z_][\w.]*)$/);
  if (!w) return [];
  const q = w[1].toUpperCase(), from = caret - w[1].length;
  const tables: Suggestion[] = ["Tasks", "Resources"].filter(n => n.toUpperCase().startsWith(q)).map(n => ({ label: n, insert: `${n}[`, about: n === "Tasks" ? "Every task of the plan, one row each" : "Every resource of the plan", from }));
  const names: Suggestion[] = cat.names.filter(([n]) => n.toUpperCase().startsWith(q)).map(([n, d]) => ({ label: n, insert: n, about: d, from }));
  const fns: Suggestion[] = cat.functions.filter(f => f[0].startsWith(q)).sort((a, b) => a[0].length - b[0].length).map(f => ({ label: f[0], insert: `${f[0]}(`, about: f[2], from }));
  return [...tables, ...names, ...fns].slice(0, 10);
};

/** The function call the caret is inside and which argument (0-based), for the signature hint. */
export const callAt = (t: string, caret: number): { name: string; arg: number } | null => {
  const s = blankStrings(t.slice(0, caret));
  let depth = 0, arg = 0;
  for (let i = s.length - 1; i >= 0; i--) {
    const ch = s[i];
    if (ch === ")" || ch === "}" || ch === "]") depth++;
    else if (ch === "{" || ch === "[") depth--;
    else if (ch === "(") { if (depth === 0) { const m = s.slice(0, i).match(/([A-Za-z_][\w.]*)$/); return m ? { name: m[1].toUpperCase(), arg } : null; } depth--; }
    else if (ch === "," && depth === 0) arg++;
  }
  return null;
};
/** A signature split into its arguments ("SUM(number1, [number2], ...)" → ["number1", "[number2]", "..."]). */
export const sigArgs = (sig: string) => { const m = sig.match(/\((.*)\)/); return m && m[1].trim() ? m[1].split(/,\s*/) : []; };

/** F4 on the reference at the caret: A1 → $A$1 → A$1 → $A1 → A1. Returns the new text and caret. */
export const cycleAnchor = (t: string, caret: number): [string, number] => {
  const r = refs(t).find(x => caret >= x.start && caret <= x.end);
  if (!r) return [t, caret];
  const part = t.slice(r.start, r.end).replace(/(\$?)([A-Z]{1,3})(\$?)(\d+)/gi, (_m, a, c, b, d) => (a && b ? `${c}$${d}` : !a && b ? `$${c}${d}` : a && !b ? `${c}${d}` : `$${c}$${d}`));
  return [t.slice(0, r.start) + part + t.slice(r.end), r.start + part.length];
};

/** What an error value means, in plain words. */
export const ERRORS: Record<string, string> = {
  "#NAME?": "Unknown function or name. Check the spelling; plan data is written like Tasks[Finish] or ProjectFinish.",
  "#REF!": "A reference points at cells that were deleted.",
  "#VALUE!": "A value of the wrong kind, such as text where a number is needed.",
  "#DIV/0!": "Division by zero or by an empty cell.",
  "#N/A": "Not found: the lookup value is not in the list.",
  "#SPILL!": "The result needs more cells, but some of them are not empty. Clear the cells below or to the right.",
  "#NUM!": "A number is out of range.",
  "#CALC!": "Nothing to show, for example FILTER found no rows. Give FILTER a third argument such as \"None\".",
  "#NULL!": "Two ranges that do not meet.",
};
