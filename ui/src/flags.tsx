/// <reference types="vite/client" />
// Country flags for vendors, crews and people (Sheets cells with the Flag format, the Resource
// Sheet's Country). A country is typed as a code (DE, de) or a name (Germany); the flag images
// come from flag-icons (MIT) and load only when shown.
import { Show, createResource } from "solid-js";

const FILES = import.meta.glob("../../node_modules/flag-icons/flags/4x3/*.svg", { query: "?url", import: "default" }) as Record<string, () => Promise<string>>;
const file = (code: string) => FILES[`../../node_modules/flag-icons/flags/4x3/${code}.svg`];
const names = (() => { try { return new Intl.DisplayNames(["en"], { type: "region" }); } catch { return null; } })();
const EXTRA: Record<string, string> = { "gb-eng": "England", "gb-sct": "Scotland", "gb-wls": "Wales", "gb-nir": "Northern Ireland", eu: "European Union", un: "United Nations" };
const ALIAS: Record<string, string> = {
  uk: "gb", "great britain": "gb", britain: "gb", england: "gb-eng", scotland: "gb-sct", wales: "gb-wls", usa: "us", america: "us", "united states of america": "us",
  uae: "ae", emirates: "ae", korea: "kr", "south korea": "kr", "north korea": "kp", russia: "ru", turkey: "tr", holland: "nl", "czech republic": "cz",
  "ivory coast": "ci", vietnam: "vn", iran: "ir", syria: "sy", laos: "la", bolivia: "bo", venezuela: "ve", tanzania: "tz", taiwan: "tw", ksa: "sa", prc: "cn", china: "cn",
};
/** Every code with a flag, and its English name. */
export const COUNTRIES: [string, string][] = Object.keys(FILES).map(k => k.slice(k.lastIndexOf("/") + 1, -4))
  .map(c => [c, EXTRA[c] ?? (c.length === 2 ? names?.of(c.toUpperCase()) ?? c.toUpperCase() : "")] as [string, string])
  .filter(([c, n]) => n && n !== c.toUpperCase() || EXTRA[c]).sort((a, b) => a[1].localeCompare(b[1]));
const BY_NAME = new Map(COUNTRIES.map(([c, n]) => [n.toLowerCase(), c]));

/** The flag code for what was typed (a code or a name), or "". */
export const countryCode = (s: string): string => {
  const t = s.trim().toLowerCase();
  if (!t) return "";
  if (file(t)) return t;
  return ALIAS[t] ?? BY_NAME.get(t) ?? "";
};
/** The country's name for what was typed (a code becomes its name). */
export const countryName = (s: string) => { const c = countryCode(s); return c ? COUNTRIES.find(x => x[0] === c)?.[1] ?? s : s; };

/** A flag (4:3) at the text's height; nothing when the country is unknown. */
export const Flag = (p: { country: string; h?: number }) => {
  const [url] = createResource(() => countryCode(p.country), c => (c && file(c) ? file(c)() : Promise.resolve("")));
  return <Show when={url()}><img class="flag" src={url()} alt={countryName(p.country)} style={{ height: `${p.h ?? 12}px`, width: `${((p.h ?? 12) * 4) / 3}px` }} /></Show>;
};
/** A flag with the country's name, as shown in cells. */
export const FlagText = (p: { country: string }) => (
  <span class="flagtext"><Flag country={p.country} /><span>{countryCode(p.country) ? countryName(p.country) : p.country}</span></span>
);
/** The choices for a country input (a <datalist id="countries">). */
export const CountryList = () => <datalist id="countries">{COUNTRIES.map(([, n]) => <option value={n} />)}</datalist>;
