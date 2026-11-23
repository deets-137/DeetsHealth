// Config › Hospital (HOSPITAL-CONFIG.md; the owner, 2026-11-15: every section now, so the page
// shows what is left to build; one hospital for every EHR source). The hospital's own lists and
// numbers, in the Config frame (Read | Edit, the versions, Discard | Save), as tabs of tables
// that are edited in place (the one table: data-table.ts, TABLES.md). A save makes a new active version; each
// call takes the active version when it opens, and keeps it to its end.
//
// Some lists are kept but read by no call yet: each one's note says so ("Not used by calls
// yet"). No row is protected data: a row is a fact about the hospital. Every size, color and
// timing is a token.
import {
  importPlaces,
  saveHospital,
  type FacilityRow,
  type HospitalSettings,
  type HospitalValues,
  type HoursRow,
  type ImportedPlaces,
  type LocationRow,
  type PatientKind,
  type ReasonSystem,
  type BookingRule,
  type RuleOutcome,
  type SpecialtyRow,
  type VisitTypeRow,
} from "../agent/api";
import { hospitalSettings } from "../agent/stores";
import { reducedMotion, tokenMs } from "../tokens";
import { settingsView, type Kit } from "./config-kit";
import { describeEpicOid, recordPrefix } from "./epic-oid";
import { fillGrid, fillTable, heightsOf, type Unit } from "./list-window";
import { el } from "./page";
import { placePill } from "./subnav";
import {
  addButton,
  addTo,
  dual,
  field,
  groupRow,
  growAll,
  listTools,
  refilter,
  row,
  rowTools,
  table,
  titleLine,
  track,
  type Column,
  type DataTable,
} from "./data-table";
import { citiesIn, placeSearch, US_STATES } from "./us-states";
import type { View } from "./subnav";
import { remember, remembered, tabBar } from "./tab-bar";

type Draft = HospitalValues;
type Key = keyof HospitalValues;
type ListKey = "visit_type_list" | "diagnosis_list" | "specialty_list";
type Limits = HospitalSettings["limits"];
// Specialties has a tab of its own (the owner, W1 A, 2026-11-15): a long list never sits under
// another one (TABLES.md §6.3).
// The Booking rules moved to Rules › Booking (the owner's 2A, 2026-11-22): one place for every
// rule. That view is this one with the rules tab alone (bookingRulesView).
const TABS = ["visits", "times", "places", "specialties", "booking", "rules"] as const;
type Tab = (typeof TABS)[number];
const TAB_WORDS: Record<Tab, string> = {
  visits: "Visits",
  times: "Times",
  places: "Places",
  specialties: "Specialties",
  booking: "Booking",
  rules: "Rules",
};
/** Each tab's lists, in the page's order. */
const TAB_KEYS: Record<Tab, Key[]> = {
  visits: ["visit_type_list", "visit_types", "visit_groups", "diagnosis_list", "reasons"],
  times: ["time_zone", "times_of_day", "hours", "closed_days"],
  places: ["facilities", "locations"],
  specialties: ["specialty_list", "specialties"],
  booking: ["slots_offered", "narrow_over", "groups_named", "past_days", "established_years", "sooner_with_others", "ahead_windows", "booking_note_label"],
  rules: ["booking_rules"],
};

// ---- The search box (TABLES.md §6.3: W2 A filter, W3 B the shown tab, W4 A Ctrl+F)

/** The name of the search's marks (config.css › ::highlight). */
const HIGHLIGHT = "deets-search";

/** A row's words for the search: each of its own words and lists of words, and `more`. */
function textOf(o: object, ...more: string[]): string {
  const out = [...more];
  for (const v of Object.values(o)) {
    if (typeof v === "string") out.push(v);
    else if (Array.isArray(v)) for (const x of v) if (typeof x === "string") out.push(x);
  }
  return out.join("\n");
}

/** The search box's words, and which rows match them. A row's first answer stays until the
 * words change, so a row being edited never leaves under the cursor; a row added since the
 * words changed shows until they change again. */
class Search {
  private words: string[] = [];
  private result = new WeakMap<object, boolean>();
  private kept = new WeakSet<object>();

  set(query: string) {
    this.words = query.toLowerCase().split(/\s+/).filter(Boolean);
    this.result = new WeakMap();
    this.kept = new WeakSet();
  }

  get on(): boolean {
    return this.words.length > 0;
  }

  get terms(): readonly string[] {
    return this.words;
  }

  keep(item: object) {
    this.kept.add(item);
  }

  /** Every search word is in `text`, in any case. */
  matches(text: string): boolean {
    const t = text.toLowerCase();
    return this.words.every((w) => t.includes(w));
  }

  /** Whether a row shows: no words, a row added since, or its words match. */
  shows(item: object, text: () => string): boolean {
    if (!this.on || this.kept.has(item)) return true;
    let r = this.result.get(item);
    if (r === undefined) {
      r = this.matches(text());
      this.result.set(item, r);
    }
    return r;
  }
}

/** Marks each search word in the words shown under `root` (the CSS Custom Highlight API: the
 * page's own text is not changed). A browser without it shows the rows without marks. */
function markMatches(root: HTMLElement | null, terms: readonly string[]) {
  const css = CSS as unknown as { highlights?: Map<string, unknown> };
  const make = (window as unknown as { Highlight?: new (...r: Range[]) => unknown }).Highlight;
  if (!css.highlights || !make) return;
  if (!root || !terms.length) {
    css.highlights.delete(HIGHLIGHT);
    return;
  }
  const ranges: Range[] = [];
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    acceptNode: (n) =>
      n.parentElement?.closest(".prompt-words, .dt-text, .people-text, .place-chip, .dt-group") ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT,
  });
  for (let n = walker.nextNode(); n; n = walker.nextNode()) {
    const text = (n.nodeValue ?? "").toLowerCase();
    for (const w of terms) {
      for (let at = text.indexOf(w); at >= 0; at = text.indexOf(w, at + w.length)) {
        const r = new Range();
        r.setStart(n, at);
        r.setEnd(n, at + w.length);
        ranges.push(r);
      }
    }
  }
  css.highlights.set(HIGHLIGHT, new make(...ranges));
}

/** The "Not in a facility" group's key in the motion and the heights (it has no row of its own). */
const NONE_GROUP = {};
const DAYS: [string, string][] = [
  ["mon", "Monday"],
  ["tue", "Tuesday"],
  ["wed", "Wednesday"],
  ["thu", "Thursday"],
  ["fri", "Friday"],
  ["sat", "Saturday"],
  ["sun", "Sunday"],
];
const NOT_USED = "Not used by calls yet: kept for when the agent can use it.";
const HH_MM = /^([01]\d|2[0-3]):[0-5]\d$/;
/** An OID as a FHIR system, or empty: ehr's hospital.rs › oid. */
const OID = /^(urn:oid:\d+(\.\d+)*)?$/;
/** A reason's code system, by name (ehr's hospital.rs › REASON_SYSTEMS). */
const REASON_SYSTEMS: [ReasonSystem, string][] = [
  ["", "None"],
  ["snomed", "SNOMED CT"],
  ["icd10cm", "ICD-10-CM"],
  ["hospital", "The hospital's diagnoses"],
];
const DATE = /^\d{4}-\d{2}-\d{2}$/;
/** A list's tools: its order is kept and used, and a row can go (TABLES.md §3.10). */
const LIST = { move: true, remove: true };
/** A visit type's Patients column (BOOKING-RULES.md §3.4): its value and its words. */
const PATIENT_KINDS: [PatientKind, string][] = [
  ["either", "Either"],
  ["new", "New"],
  ["established", "Established"],
];
/** A Booking rule's outcomes (BOOKING-RULES.md §3.5; a draft for the owner to edit), and what
 * each does. */
const RULE_OUTCOMES: [RuleOutcome, string][] = [
  ["book", "Book"],
  ["book_and_flag", "Book and flag for a referral check"],
  ["pcp_only", "Book with the PCP only"],
  ["referral_needed", "Referral needed"],
  ["hand_off", "Hand off"],
];
const RULE_HELP: Record<RuleOutcome, string> = {
  book: "The caller is offered times now.",
  book_and_flag: "The caller is offered times now. A booking files a staff flag (referral_check), with the visit's number, so staff check the referral before the visit. The caller hears nothing of it.",
  pcp_only: "Only the times with the caller's PCP are offered; no sooner time with another clinician. A caller with no PCP on file is offered every time.",
  referral_needed: "No time is offered. The agent says the visit needs a referral from primary care, and offers a primary care visit or a member of staff; a caller who says they have a referral goes to staff.",
  hand_off: "No time is offered. The caller goes to a member of staff, with a staff flag (booking_rule_hand_off) that holds the caller's words for the reason.",
};
/** A rule's Patients column: "either" reads as Any here. */
const RULE_PATIENTS: [PatientKind, string][] = [
  ["either", "Any"],
  ["new", "New"],
  ["established", "Established"],
];

const PATIENT_HELP: Record<PatientKind, string> = {
  either: "Any caller. No search of the caller's visits for this type.",
  new: "Only a caller new to the specialty of the slot's department: not seen by it within Booking › Years for an established patient.",
  established: "Only a caller the specialty of the slot's department has seen within Booking › Years for an established patient.",
};

// ---- Places › Locations (HOSPITAL-CONFIG.md §8.2)
const PLACES_VIEWS = ["cards", "table"] as const;
type PlacesView = (typeof PLACES_VIEWS)[number];
const PLACES_VIEW_KEY = "deets.console.hospital.places";
/** The wave's last step: rows after it come in with it, so a long table never waits. */
const WAVE_LAST = 12;
/** The specialties a card names before "n more". */
const CHIPS = 3;
/** An import's mark on a row, until Save or Discard. */
type Mark = "new" | "changed" | "gone";

/** The Places tab's own state: it lives across the redraws of one view. */
type PlacesUi = {
  view: PlacesView;
  /** The facility drilled into, by `facilityKey`; "none" for the departments in none. */
  open: string | null;
  /** The stage comes in as a wave on the next draw. */
  wave: boolean;
  /** The stage drawn last, for the fade out before a drill. */
  stage?: HTMLElement;
  /** The Cards | Table toggle drawn last: its pill slides before the redraw. */
  toggle?: HTMLElement;
  marks: WeakMap<object, Mark>;
  /** The active version's values, while an older one is shown: its changed cards are marked. */
  older: HospitalValues | null;
  /** The searches in the Import panel ("WI", "Verona, WI"): kept while the view lives. */
  importEntries: string[];
  importSay: string;
  go: (open: string | null) => void;
  /** A facility just added: the page drills into it, the cursor in its name (TABLES.md §3.7). */
  fresh?: FacilityRow;
  openNew: (f: FacilityRow) => void;
  setView: (v: PlacesView) => void;
  runImport: (d: Draft, searches: string[], say: HTMLElement, button: HTMLButtonElement) => void;
  /** The search box's words, for every tab (TABLES.md §6.3). */
  search: Search;
  /** Marks the search words again on the next frame (rows drawn by a long list's window). */
  markSoon: () => void;
};
/** The most searches one import runs: ehr's ImportPlaces takes 1 to 5. */
const PLACE_SEARCHES = 5;
/** The most suggestions the Import panel shows at once. */
const SUGGESTIONS = 6;

/** A version saved before a row gained its fields (a department's facility, DEP ID, specialty
 * and hours, 2026-11-15; a visit type's patients, 2026-11-16) holds rows without them: fill
 * each with its empty value, as ehr's hospital.rs does (`#[serde(default)]`). Without this the
 * page's checks read a missing field and the page never draws. A number saved before it
 * existed takes the Default's, as ehr's `from_pairs` takes the file's. */
function withRowDefaults(v: HospitalValues): HospitalValues {
  const stored = v as Omit<HospitalValues, "facilities" | "locations" | "visit_types" | "specialties" | "established_years" | "sooner_with_others" | "booking_rules"> & {
    booking_rules?: Partial<BookingRule>[];
    facilities?: Partial<FacilityRow>[];
    locations?: Partial<LocationRow>[];
    visit_types?: Partial<VisitTypeRow>[];
    specialties?: Partial<SpecialtyRow>[];
    established_years?: number;
    sooner_with_others?: boolean;
  };
  v.visit_types = (stored.visit_types ?? []).map((t) => ({ code: "", record: "", spoken: "", online: false, book: false, patients: "either", ...t }));
  v.specialties = (stored.specialties ?? []).map((s) => ({ code: "", spoken: "", words: [], primary_care: false, ...s }));
  v.established_years = stored.established_years ?? 3;
  v.sooner_with_others = stored.sooner_with_others ?? true;
  // A version saved before the Booking rules (2026-11-16) has none: every visit is Book.
  v.booking_rules = (stored.booking_rules ?? []).map((r) => ({ specialty: "", department: "", visit_type: "", provider: "", patients: "either", outcome: "book", ...r }));
  v.facilities = (stored.facilities ?? []).map((f) => ({ record: "", spoken: "", id: "", code: "", address: "", phone: "", ...f }));
  v.locations = (stored.locations ?? []).map((l) => ({
    record: "",
    spoken: "",
    id: "",
    address: "",
    phone: "",
    facility: "",
    code: "",
    specialty: "",
    ...l,
    hours: l.hours ?? [],
  }));
  return v;
}

/** A facility's key on this page: its FHIR id, else its record's name. */
const facilityKey = (f: FacilityRow) => (f.id.trim() ? `id:${f.id.trim()}` : `name:${f.record.trim().toLowerCase()}`);

/** The facility a key names in this draft; null when it is gone (Back to the cards). "new": the
 * facility just added, which has no name or id to key it by yet. */
function resolveOpen(d: Draft, open: string | null, fresh?: FacilityRow): FacilityRow | "none" | null {
  if (open === "none") return "none";
  if (open === "new") return fresh && d.facilities.includes(fresh) ? fresh : null;
  return open ? (d.facilities.find((f) => facilityKey(f) === open) ?? null) : null;
}

/** The same place: by FHIR id when the row has one, else by the record's name (in any case). */
function samePlace(a: { id: string; record: string }, b: { id: string; record: string }): boolean {
  if (a.id.trim() && b.id.trim()) return a.id.trim() === b.id.trim();
  return a.record.trim().toLowerCase() === b.record.trim().toLowerCase() && !!a.record.trim();
}

/** Merges an import into the draft (HOSPITAL-CONFIG.md §8.2): Epic's facts replace ours, the
 * spoken names and the caller's words stay, new and changed rows are marked, a row Epic did not
 * return stays, marked "gone". Returns the words for the status line, or throws when the
 * specialty list's OID differs from the one set. Counts only, never a value. */
function mergePlaces(d: Draft, got: ImportedPlaces, marks: WeakMap<object, Mark>, limits: Limits): string {
  const v = got.values;
  if (v.specialty_list && d.specialty_list && v.specialty_list !== d.specialty_list) {
    throw new Error("Epic's specialty list is not the one set on this page: clear the Specialty list's OID to take Epic's");
  }
  const counts = { facilities: [0, 0], departments: [0, 0], specialties: 0, gone: 0 };
  const merge = <T extends { id: string; record: string }>(mine: T[], theirs: T[], keep: (old: T, next: T) => T, tally: number[]) => {
    const matched = new Set<T>();
    for (const t of theirs) {
      const old = mine.find((m) => !matched.has(m) && samePlace(m, t));
      if (!old) {
        mine.push(t);
        matched.add(t);
        marks.set(t, "new");
        tally[0]++;
        continue;
      }
      matched.add(old);
      const next = keep(old, t);
      if (JSON.stringify(next) !== JSON.stringify(old)) {
        Object.assign(old, next);
        if (marks.get(old) !== "new") marks.set(old, "changed");
        tally[1]++;
      }
    }
    for (const m of mine) {
      if (!matched.has(m)) {
        marks.set(m, "gone");
        counts.gone++;
      }
    }
  };
  merge(d.facilities, v.facilities, (old, t) => ({ ...t, spoken: old.spoken }), counts.facilities);
  merge(d.locations, v.locations, (old, t) => ({ ...t, spoken: old.spoken }), counts.departments);
  if (v.specialty_list && !d.specialty_list) d.specialty_list = v.specialty_list;
  for (const s of v.specialties) {
    if (d.specialties.some((x) => x.code === s.code)) continue;
    d.specialties.push(s);
    marks.set(s, "new");
    counts.specialties++;
  }
  const over = d.facilities.length > limits.places || d.locations.length > limits.places;
  const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;
  return [
    `Epic: ${plural(v.facilities.length, "facility", "facilities")}, ${plural(v.locations.length, "department", "departments")}`,
    `${counts.facilities[0] + counts.departments[0]} new, ${counts.facilities[1] + counts.departments[1]} changed`,
    counts.gone ? `${counts.gone} on the page not in Epic (kept)` : "",
    counts.specialties ? plural(counts.specialties, "new specialty", "new specialties") : "",
    got.others ? `${got.others} other locations left out` : "",
    got.other_specialty_lists ? `${got.other_specialty_lists} departments' specialties are in another list (left empty)` : "",
    plural(got.reads, "read", "reads"),
    over ? `more than ${limits.places} rows: remove some before Save` : "",
    "Nothing is saved until Save",
  ]
    .filter(Boolean)
    .join(". ");
}

/** A cell of words: "a, b, c" in the box, a list in the draft. */
const words = (list: string[]) => list.join(", ");
const unwords = (text: string) =>
  text
    .split(",")
    .map((w) => w.trim())
    .filter(Boolean);

/** The first row whose `name` repeats an earlier one, in any case (1-based), or 0. */
function twice<T>(rows: T[], name: (r: T) => string): number {
  const seen: string[] = [];
  for (let i = 0; i < rows.length; i++) {
    const n = name(rows[i]).trim().toLowerCase();
    if (!n) continue;
    if (seen.includes(n)) return i + 1;
    seen.push(n);
  }
  return 0;
}

/** The first row (1-based) that names a place an earlier row names, or 0: by FHIR id when the row
 * has one, else by the record's name (ehr's hospital.rs › twice_place). */
function twicePlace(rows: { id: string; record: string }[]): number {
  const seen = new Set<string>();
  for (let i = 0; i < rows.length; i++) {
    const id = rows[i].id.trim();
    const key = id ? `id:${id}` : `name:${rows[i].record.trim().toLowerCase()}`;
    if (key === "name:") continue;
    if (seen.has(key)) return i + 1;
    seen.add(key);
  }
  return 0;
}

/** What does not fit, in the page's words, or null: the rules of ehr's hospital.rs, which
 * checks again on save. Names a list and a row, never a value. */
function problem(d: Draft, l: Limits): string | null {
  const one = (text: string, needed: boolean) =>
    (needed && !text.trim()) || text.length > l.cell || /[\u0000-\u001f\u007f-\u009f]/.test(text);
  const many = (list: string[]) => list.length > l.words || list.some((w) => one(w, true));
  const each = <T>(name: string, rows: T[], check: (r: T) => string | null, max = l.rows): string | null => {
    if (rows.length > max) return `${name}: at most ${max} rows`;
    for (let i = 0; i < rows.length; i++) {
      const p = check(rows[i]);
      if (p) return `${name}, row ${i + 1}: ${p}`;
    }
    return null;
  };
  const list = (name: string, oid: string) => (OID.test(oid) && oid.length <= l.cell + 8 ? null : `${name}: an OID such as urn:oid:1.2.840.114350…, or empty`);
  const needsList = <T>(name: string, rows: T[], oid: string, uses: (r: T) => boolean, what: string) => {
    const i = oid ? -1 : rows.findIndex(uses);
    return i < 0 ? null : `${name}, row ${i + 1}: ${what}`;
  };
  const checks: (() => string | null)[] = [
    () => (d.time_zone ? null : "Time zone: choose one"),
    () => list("Visit type list", d.visit_type_list),
    () => list("Diagnosis list", d.diagnosis_list),
    () => list("Specialty list", d.specialty_list),
    () =>
      each("Visit types", d.visit_types, (r) => {
        if (one(r.record, true)) return "the record's name is empty or too long";
        if ([r.spoken, r.code].some((x) => one(x, false))) return "a cell is too long";
        return null;
      }),
    () => needsList("Visit types", d.visit_types, d.visit_type_list, (r) => !!r.code.trim(), "an ID needs the visit type list's OID"),
    () => needsList("Specialties", d.specialties, d.specialty_list, (r) => !!r.code.trim(), "an ID needs the specialty list's OID"),
    () => needsList("Reasons", d.reasons, d.diagnosis_list, (r) => r.system === "hospital", "the hospital's list needs its OID"),
    () => (twice(d.visit_types, (r) => r.record) ? `Visit types, row ${twice(d.visit_types, (r) => r.record)}: the record's name is there twice` : null),
    () =>
      each("Visit groups", d.visit_groups, (g) =>
        one(g.name, true) ? "the name is empty or too long" : many(g.words) || many(g.visit_types) ? "too many words, or one too long" : null,
      ),
    () => (twice(d.visit_groups, (g) => g.name) ? `Visit groups, row ${twice(d.visit_groups, (g) => g.name)}: the name is there twice` : null),
    () =>
      each("Reasons", d.reasons, (r) =>
        !r.words.length
          ? "needs the caller's words"
          : many(r.words)
            ? "too many words, or one too long"
            : r.code.trim() && !r.system
              ? "a code needs its code system"
              : [r.code, r.visit_type, r.specialty].some((x) => one(x, false))
                ? "a cell is too long"
                : null,
      ),
    () =>
      d.times_of_day.length > l.times
        ? `Times of day: at most ${l.times}`
        : each("Times of day", d.times_of_day, (t) => {
            if (!/^[a-z -]{1,40}$/.test(t.name)) return "the name holds only a to z, spaces and -";
            if (!HH_MM.test(t.from)) return "the start is not HH:MM";
            const [h, m] = t.from.split(":").map(Number);
            if (!(t.hours >= 1) || h * 60 + m + t.hours * 60 > 24 * 60) return "the length ends after midnight or is 0";
            return null;
          }),
    () => (twice(d.times_of_day, (t) => t.name) ? `Times of day, row ${twice(d.times_of_day, (t) => t.name)}: the name is there twice` : null),
    () =>
      each("Hours", d.hours, (h) =>
        !HH_MM.test(h.opens) || !HH_MM.test(h.closes) ? "a time is not HH:MM" : h.opens >= h.closes ? "it closes before it opens" : null,
      ),
    () => each("Closed days", d.closed_days, (c) => (!DATE.test(c.date) || Number.isNaN(Date.parse(c.date)) ? "the date is not YYYY-MM-DD" : one(c.name, false) ? "the name is too long" : null)),
    () =>
      each(
        "Facilities",
        d.facilities,
        (r) => (one(r.record, true) ? "the record's name is empty or too long" : [r.spoken, r.id, r.code, r.address, r.phone].some((x) => one(x, false)) ? "a cell is too long" : null),
        l.places,
      ),
    () => (twicePlace(d.facilities) ? `Facilities, row ${twicePlace(d.facilities)}: the facility is there twice` : null),
    () =>
      each(
        "Departments",
        d.locations,
        (r) => {
          if (one(r.record, true)) return "the record's name is empty or too long";
          if ([r.spoken, r.id, r.address, r.phone, r.facility, r.code, r.specialty].some((x) => one(x, false))) return "a cell is too long";
          if (r.facility && !d.facilities.some((f) => f.id.trim() === r.facility.trim())) return "its facility is not on the facility list";
          if (r.specialty && !d.specialty_list) return "a specialty ID needs the specialty list's OID";
          return null;
        },
        l.places,
      ),
    () => (twicePlace(d.locations) ? `Departments, row ${twicePlace(d.locations)}: the department is there twice` : null),
    () =>
      each("Specialties", d.specialties, (s) =>
        one(s.spoken, true) ? "the spoken name is empty or too long" : many(s.words) ? "too many words, or one too long" : one(s.code, false) ? "a cell is too long" : null,
      ),
    // A rule names rows on the other lists (ehr's hospital.rs › from_pairs).
    () =>
      each("Booking rules", d.booking_rules, (r) => {
        if ([r.specialty, r.department, r.visit_type, r.provider].some((x) => one(x, false))) return "a cell is too long";
        if (r.specialty && !d.specialties.some((s) => s.code.trim() === r.specialty.trim())) return "its specialty is not on the Specialties list";
        const same = (a: string, b: string) => a.trim().toLowerCase() === b.trim().toLowerCase();
        if (r.department && !d.locations.some((x) => same(x.record, r.department))) return "its department is not on the Places list";
        if (r.visit_type && !d.visit_types.some((x) => same(x.record, r.visit_type))) return "its visit type is not on the Visit types list";
        return null;
      }),
    () => {
      const w = d.ahead_windows;
      if (!w.length || w.length > l.windows) return `Search windows: 1 to ${l.windows}`;
      if (w.some((n) => !Number.isInteger(n) || n < 1 || n > 365)) return "Search windows: each is 1 to 365 days";
      if (w.some((n, i) => i > 0 && n <= w[i - 1])) return "Search windows: each is longer than the one before";
      return null;
    },
  ];
  for (const c of checks) {
    const p = c();
    if (p) return p;
  }
  return null;
}

/** Each list of a saved or active version as JSON, made once: those versions are never edited
 * (the kit edits a copy, the draft), and the check runs on each frame of typing. */
const savedJson = new WeakMap<Draft, Map<Key, string>>();

function differing(a: Draft, b: Draft): Set<string> {
  const out = new Set<string>();
  let cache = savedJson.get(a);
  if (!cache) savedJson.set(a, (cache = new Map()));
  for (const k of Object.keys({ ...a, ...b }) as Key[]) {
    let was = cache.get(k);
    if (was === undefined) cache.set(k, (was = JSON.stringify(a[k])));
    if (was !== JSON.stringify(b[k])) out.add(k);
  }
  return out;
}

/** One view of the hospital's settings: Config › Hospital (every tab but Rules), or Rules ›
 * Booking (the rules tab alone). Both edit the same versions. A save sends this view's own
 * lists over the newest active version, so one view's save never undoes the other's. */
type HospitalSpec = { id: string; title: string; tabs: readonly Tab[]; tabKey: string };

const HOSPITAL: HospitalSpec = {
  id: "hospital",
  title: "Hospital",
  tabs: TABS.filter((t) => t !== "rules"),
  tabKey: "deets.console.hospital.tab",
};

/** Rules › Booking (BOOKING-RULES.md §5.5): the Booking rules table, in the Config frame. */
export const bookingRulesView = (): View =>
  hospitalView({ id: "booking", title: "Booking", tabs: ["rules"], tabKey: "deets.console.rules.booking.tab" });

export const hospitalView = (spec: HospitalSpec = HOSPITAL): View => {
  const TABS = spec.tabs;
  const TAB_KEY = spec.tabKey;
  const ownKeys = TABS.flatMap((t) => TAB_KEYS[t]);
  // This view's lists from the draft; the rest from the newest active version the store holds.
  // The other view (Config › Hospital or Rules › Booking) may have saved since this one drew,
  // so the checks and a save use its lists, never this view's stale copy.
  const whole = (d: Draft): Draft => {
    const now = hospitalSettings(0).get().value?.active;
    if (!now) return d;
    const out = withRowDefaults(structuredClone(now)) as Record<Key, unknown>;
    for (const k of ownKeys) out[k] = d[k];
    return out as Draft;
  };
  let tab: Tab = remembered(TAB_KEY, TABS, TABS[0]);
  let slideTab: string | undefined;
  let latest: HospitalSettings | undefined;
  let kitNow: Kit | undefined;
  let draftNow: Draft | undefined;
  let swapping = false;

  // Places › Locations: a drill in or out fades the stage out as a page swap does (nav.ts,
  // --dur-leave), then draws the new stage, which comes in as Chat's wave (chat.css › the wave:
  // --dur-wave-in, --wave-step, --wave-rise, --ease-wave). No motion with reduced motion.
  // `wait`: how long before the redraw; the view toggle waits for its pill to land (--dur-pill),
  // so the redraw never cuts its slide short.
  const swap = (change: () => void, wait = tokenMs("--dur-leave")) => {
    if (swapping) return;
    const finish = () => {
      swapping = false;
      change();
      ui.wave = !reducedMotion();
      kitNow?.rerender(true);
    };
    const stage = ui.stage;
    if (!stage?.isConnected || reducedMotion()) return finish();
    swapping = true;
    stage.classList.add("place-leaving");
    window.setTimeout(finish, Math.max(wait, tokenMs("--dur-leave")));
  };
  const ui: PlacesUi = {
    view: remembered(PLACES_VIEW_KEY, PLACES_VIEWS, "cards"),
    open: null,
    wave: false,
    marks: new WeakMap(),
    older: null,
    importEntries: [],
    importSay: "",
    go: (open) => swap(() => (ui.open = open)),
    openNew: (f) =>
      swap(() => {
        ui.fresh = f;
        ui.open = "new";
        if (ui.view !== "cards") {
          ui.view = "cards";
          remember(PLACES_VIEW_KEY, "cards");
        }
        // After the redraw that follows this change: the cursor in the facility's first box.
        queueMicrotask(() => ui.stage?.querySelector<HTMLTextAreaElement>("textarea")?.focus());
      }),
    setView: (v) => {
      if (swapping || v === ui.view) return;
      // The pill slides to the new choice now, as Read | Edit's does (subnav.ts › placePill);
      // the stage fades out under it, and the redraw comes when the pill has landed.
      const bar = ui.toggle;
      const to = bar?.querySelector<HTMLElement>(`[data-value="${v}"]`);
      const pill = bar?.querySelector<HTMLElement>(".tab-pill");
      if (bar && to && pill) {
        bar.querySelectorAll<HTMLElement>("[data-value]").forEach((b) => b.setAttribute("aria-selected", String(b === to)));
        placePill(pill, to, !reducedMotion());
      }
      swap(() => {
        ui.view = v;
        remember(PLACES_VIEW_KEY, v);
      }, tokenMs("--dur-pill"));
    },
    runImport: (d, searches, say, button) => {
      const show = (text: string) => {
        say.textContent = text;
        say.hidden = !text;
      };
      button.disabled = true;
      show("Importing from Epic");
      importPlaces(searches)
        .then((got) => {
          // A version picked, a Discard or a Save while it ran: this draft is gone.
          if (!kitNow || !latest || draftNow !== d) return show("The page changed while Epic answered: import again");
          ui.importSay = mergePlaces(d, got, ui.marks, latest.limits);
          kitNow.changed();
          kitNow.rerender();
        })
        .catch((e: unknown) => show(`Not imported: ${e instanceof Error && e.message ? e.message : "the import failed"}`))
        .finally(() => (button.disabled = false));
    },
    search: new Search(),
    markSoon: () => {
      if (marking) return;
      marking = true;
      requestAnimationFrame(() => {
        marking = false;
        markMatches(sections, ui.search.terms);
      });
    },
  };

  // ---- the search box (TABLES.md §6.3): one box for the view's life, put on the tab bar's line
  // at each draw, so a search's redraw never takes the cursor out of it.
  let marking = false;
  /** The tab's sections drawn last, and how to draw them again for a search. */
  let sections: HTMLElement | null = null;
  let redrawSections: (() => void) | null = null;
  const searchWrap = el(`
    <div class="cfg-search" role="search">
      <input class="cfg-field cfg-search-box" type="search" spellcheck="false" autocomplete="off"
        placeholder="Search this tab" aria-label="Search this tab" aria-keyshortcuts="Control+F Meta+F" />
    </div>`);
  const searchBox = searchWrap.querySelector<HTMLInputElement>("input")!;
  searchBox.title = "Ctrl+F. Press Ctrl+F again for the browser's own find. Esc clears.";
  let searchQueued = false;
  searchBox.addEventListener("input", () => {
    // One redraw a frame, however fast the typing.
    if (searchQueued) return;
    searchQueued = true;
    requestAnimationFrame(() => {
      searchQueued = false;
      ui.search.set(searchBox.value.trim());
      if (redrawSections && sections?.isConnected) refilter(redrawSections);
    });
  });
  searchBox.addEventListener("keydown", (e) => {
    if (e.key !== "Escape") return;
    e.preventDefault();
    if (!searchBox.value) return searchBox.blur();
    searchBox.value = "";
    searchBox.dispatchEvent(new Event("input"));
  });
  // Ctrl+F (⌘F) while this view shows: the first press goes to the box; a second press, while
  // the box has the cursor, is left to the browser, whose own find opens (the owner, W4 A).
  document.addEventListener("keydown", (e) => {
    if (!(e.ctrlKey || e.metaKey) || e.altKey || e.shiftKey || e.key.toLowerCase() !== "f") return;
    if (!searchBox.isConnected || !searchBox.offsetParent) return;
    if (document.activeElement === searchBox) return;
    e.preventDefault();
    searchBox.focus();
    searchBox.select();
  });
  let cardWatch: ResizeObserver | undefined;

  return settingsView<HospitalValues, Draft, HospitalSettings>({
    id: spec.id,
    title: spec.title,
    store: hospitalSettings,
    took: (s) => {
      latest = s;
      // A new draft (a Save, a version): the import's marks and its line were for the old one.
      ui.marks = new WeakMap();
      ui.importSay = "";
      ui.older = s.version && !s.version.active ? s.active : null;
    },
    // This view's lists over the newest active version, read again first.
    save: async (_s, d) => {
      await hospitalSettings(0).refresh();
      return saveHospital(whole(d));
    },
    fromValues: (v) => withRowDefaults(structuredClone(v)),
    differing,
    blocked: (d) => (latest ? problem(whole(d), latest.limits) : null),
    render: (s, d, kit) => render(s, d, kit),
  });

  function render(s: HospitalSettings, d: Draft, kit: Kit) {
    const card = el(`<section class="cfg-card prompt-card"></section>`);
    card.dataset.card = "hospital";
    const from = slideTab;
    slideTab = undefined;
    const bar = tabBar(
      spec.title,
      TABS.map((t): [string, string] => [t, TAB_WORDS[t]]),
      tab,
      (v) => {
        slideTab = tab;
        tab = v as Tab;
        remember(TAB_KEY, tab);
        kit.rerender(true);
      },
      from,
    );
    const bars = el(`<div class="prompt-bars"></div>`);
    // A view of one tab has no tab bar: the search box alone.
    bar.el.hidden = TABS.length < 2;
    bars.append(bar.el, searchWrap);
    const body = el(`<div class="prompt-sections"></div>`);
    kitNow = kit;
    draftNow = d;
    const err = el(`<p class="prompt-problem" role="status" hidden></p>`);
    const showProblem = () => {
      const msg = problem(whole(d), s.limits);
      err.textContent = msg ?? "";
      err.hidden = !msg;
    };
    // Once a frame while typing: the check reads every list.
    let problemQueued = false;
    const problemSoon = () => {
      if (problemQueued) return;
      problemQueued = true;
      requestAnimationFrame(() => {
        problemQueued = false;
        showProblem();
      });
    };
    const drawSections = () => {
      const p = new Parts(s.limits, d, kit, ui);
      p.onEdit = problemSoon;
      if (tab === "visits") body.replaceChildren(...p.visitTypes(), ...p.visitGroups(), ...p.reasons());
      else if (tab === "times") body.replaceChildren(...p.timeZone(), ...p.timesOfDay(), ...p.hours(), ...p.closedDays());
      else if (tab === "places") body.replaceChildren(...p.places());
      else if (tab === "specialties") body.replaceChildren(...p.specialties());
      else if (tab === "rules") body.replaceChildren(...p.rules());
      else body.replaceChildren(...p.booking());
      ui.markSoon();
    };
    drawSections();
    sections = body;
    // A search draws only the sections again; the kit gives their controls the page's mode.
    redrawSections = () => {
      drawSections();
      kit.apply([body]);
    };
    showProblem();
    card.append(bars, err, body);
    const settle = () => {
      growAll(card);
      bar.place();
    };
    requestAnimationFrame(settle);
    // One watcher for the card shown; the last draw's card is gone.
    cardWatch?.disconnect();
    cardWatch = new ResizeObserver(settle);
    cardWatch.observe(card);
    return {
      cards: [card],
      layout: "prompt",
      // An older version: each tab that holds a changed list shows a dot.
      glow: (diff: Set<string>) => {
        for (const t of TABS) bar.button(t)?.classList.toggle("prompt-tab-changed", TAB_KEYS[t].some((k) => diff.has(k)));
      },
    };
  }
};

/** The tables of one draft. Each edit changes the draft in place and tells the kit. */
class Parts {
  onEdit: () => void = () => {};

  constructor(
    private l: Limits,
    private d: Draft,
    private kit: Kit,
    private ui: PlacesUi,
  ) {}

  private edited() {
    this.kit.changed();
    this.onEdit();
  }

  // ---- the search and the long lists (TABLES.md §6)

  /** Whether a row shows under the search box's words. */
  private shows(item: object, text: () => string = () => textOf(item)): boolean {
    return this.ui.search.shows(item, text);
  }

  /** The rows of a list that show. */
  private visible<T extends object>(rows: T[], text?: (r: T) => string): T[] {
    if (!this.ui.search.on) return rows;
    return rows.filter((r) => this.shows(r, text ? () => text(r) : undefined));
  }

  /** A title's count: "12 of 403" while a search is on. */
  private count(shown: number, total: number): number | string {
    return this.ui.search.on ? `${shown} of ${total}` : total;
  }

  /** After a long list's window draws rows: the page's mode, the boxes' heights, the marks. */
  private drawnLater = (nodes: HTMLElement[]) => {
    this.kit.apply(nodes);
    growAll(nodes);
    this.ui.markSoon();
  };

  /** A list's rows into its table, windowed when long. `list` names its kept heights. */
  private fill<T extends object>(t: DataTable, list: string, rows: T[], draw: (r: T) => HTMLElement[]) {
    if (this.ui.search.on) t.wrap.dataset.empty = "No match";
    fillTable(
      t,
      rows.map((r) => ({ item: r, draw: () => draw(r) })),
      { heights: heightsOf(list), drawn: this.drawnLater },
    );
  }

  /** A one-line text cell. */
  private text(label: string, value: string, on: (v: string) => void): HTMLElement {
    return field(label, value, this.l.cell, (v) => (on(v), this.edited()));
  }

  /** A cell of words, comma separated. */
  private wordsCell(label: string, value: string[], on: (v: string[]) => void): HTMLElement {
    return field(`${label} (comma separated)`, words(value), this.l.cell * 4, (v) => (on(unwords(v)), this.edited()));
  }

  /** Off | On in a cell; "On" or "Off" in Read. */
  private onOff(label: string, value: boolean, on: (v: boolean) => void): HTMLElement {
    let now = value;
    return dual(
      this.kit.pair(value, (v) => ((now = v), on(v), this.onEdit()), label),
      () => (now ? "On" : "Off"),
    );
  }

  /** A select in a cell; its chosen words in Read. */
  private choose(options: [string, string][], value: string, on: (v: string) => void): HTMLElement {
    const select = this.kit.select(options, value, (v) => (on(v), this.onEdit()));
    return dual(select, () => select.selectedOptions[0]?.textContent ?? "");
  }

  /** A number in a cell; the number in Read. */
  private num(label: string, value: number, min: number, max: number, on: (v: number) => void): HTMLElement {
    const input = this.kit.number(value, min, max, (v) => (on(v), this.onEdit()), label);
    return dual(input, () => input.value);
  }

  /** Draws the page again after a row tool or Add. */
  private done = () => {
    this.kit.changed();
    this.kit.rerender();
  };

  /** ↑ ↓ ✕ for a row of a list (Edit only): the order matters where a note says so. */
  private moves<T extends object>(rows: T[], r: T, what: string): HTMLElement {
    return listTools(rows, r, what, this.done, { still: this.ui.search.on });
  }

  /** The "R PRC" spans of each list's rows, kept current while the list's OID is typed. */
  private prefixes = new Map<ListKey, HTMLElement[]>();

  /** A list's one OID, set once above its rows (the owner's A, 2026-11-15), and what the OID
   * says in an analyst's words (epic-oid.ts: only the forms Epic's documentation names). */
  private listBlock(title: string, key: ListKey, help: string): HTMLElement {
    const d = this.d;
    const t = table(title, ["Epic's OID for the list", "What the OID says"]);
    const says = el(`<span></span>`);
    says.textContent = describeEpicOid(d[key]);
    const oid = this.text(title, d[key], (v) => {
      d[key] = v.trim();
      says.textContent = describeEpicOid(d[key]);
      for (const p of this.prefixes.get(key) ?? []) p.textContent = recordPrefix(d[key]);
    });
    t.body.append(
      ...row(null, title, [oid, says], [
        ["The OID:", help],
        ["Where it comes from:", "The hospital's Epic team runs Epic's OID Generator utility and sends it. It differs between the hospital's test and production Epic."],
      ]),
    );
    return t.wrap;
  }

  /** A row's ID in its list, after the list's prefix ("R PRC 1003"). The prefix is shown only;
   * the OID and the ID are what is kept and sent. */
  private idCell(key: ListKey, value: string, on: (v: string) => void): HTMLElement {
    const wrap = el(`<div class="sys-id dt-cell"><span class="sys-id-file"></span></div>`);
    const prefix = wrap.firstElementChild as HTMLElement;
    prefix.textContent = recordPrefix(this.d[key]);
    this.prefixes.set(key, [...(this.prefixes.get(key) ?? []), prefix]);
    wrap.append(this.text("ID", value, on));
    return wrap;
  }

  /** Add, on a section's title line (Edit only): an empty row at the foot of the list, to fill
   * in place (TABLES.md §3.7). */
  private add<T extends object>(what: string, rows: T[], blank: () => T, max = this.l.rows): HTMLElement {
    return addButton(`Add ${what}`, () => {
      // A new row shows while a search is on: it matches nothing yet.
      const item = blank();
      this.ui.search.keep(item);
      addTo(rows, item, this.done, max);
    });
  }

  visitTypes(): HTMLElement[] {
    const d = this.d;
    const rows = this.visible(d.visit_types);
    const out = titleLine(
      "Visit types",
      this.count(rows.length, d.visit_types.length),
      "Epic's service-type. A visit whose type is on this list says the spoken name and takes the Online mark. Epic searches each type the agent may book that has an ID; the local fake books the first of each kind that fits the caller, in this order. Patients: a type for new or for established patients is offered only to a caller who fits, by the specialty of the slot's department (Booking › Years for an established patient).",
      this.add("a visit type", d.visit_types, () => ({ code: "", record: "", spoken: "", online: false, book: false, patients: "either" })),
    );
    out.push(this.listBlock("Visit type list", "visit_type_list", "The list every visit type's ID belongs to, sent with each ID to Epic's $find."));
    const t = table(
      "Visit types",
      [
        "The record's name",
        "Spoken name",
        { head: "ID", kind: "id" },
        { head: "Online", kind: "flag" },
        { head: "Agent may book", kind: "flag" },
        { head: "Patients", kind: "choice" },
      ],
      LIST,
    );
    this.fill(t, "visit-types", rows, (r) =>
        row(
          r,
          r.record || "A new visit type",
          [
            this.text("The record's name", r.record, (v) => (r.record = v)),
            this.text("Spoken name", r.spoken, (v) => (r.spoken = v)),
            this.idCell("visit_type_list", r.code, (v) => (r.code = v)),
            this.onOff("Online", r.online, (v) => (r.online = v)),
            this.onOff("Agent may book", r.book, (v) => (r.book = v)),
            this.choose(PATIENT_KINDS, r.patients, (v) => (r.patients = v as PatientKind)),
          ],
          [
            ["The record's name:", "The type's name as the record holds it. A visit of this name, in any case, takes this row."],
            ["Spoken name:", "What the caller hears for this type. Empty: the record's own words. A name that names sensitive care makes the visit private."],
            ["ID:", r.code ? "The type's ID in the visit type list, sent to Epic's $find." : "None: Epic cannot search or book this type. The local fake can."],
            ["Patients:", PATIENT_HELP[r.patients]],
          ],
          this.moves(d.visit_types, r, "this visit type"),
        ),
    );
    out.push(t.wrap);
    return out;
  }

  visitGroups(): HTMLElement[] {
    const d = this.d;
    const rows = this.visible(d.visit_groups);
    const out = titleLine(
      "Visit groups",
      this.count(rows.length, d.visit_groups.length),
      "When a caller has several visits, the agent asks which group they mean. A group whose name or words name sensitive care is never said.",
      this.add("a group", d.visit_groups, () => ({ name: "", words: [], visit_types: [] })),
    );
    const t = table("Visit groups", ["Name the caller hears", "The caller's words", "Visit types"], LIST);
    this.fill(t, "visit-groups", rows, (g) =>
        row(
          g,
          g.name || "A new group",
          [
            this.text("Name the caller hears", g.name, (v) => (g.name = v)),
            this.wordsCell("The caller's words", g.words, (v) => (g.words = v)),
            this.wordsCell("Visit types", g.visit_types, (v) => (g.visit_types = v)),
          ],
          [
            ["The caller's words:", "Other words a caller may say for this group, such as \"my checkup\"."],
            ["Visit types:", "The members, by the record's name of each visit type."],
          ],
          this.moves(d.visit_groups, g, "this group"),
        ),
    );
    out.push(t.wrap);
    return out;
  }

  reasons(): HTMLElement[] {
    const d = this.d;
    const rows = this.visible(d.reasons, (r) => textOf(r, REASON_SYSTEMS.find(([k]) => k === r.system)?.[1] ?? ""));
    const out = titleLine(
      "Reasons as codes",
      this.count(rows.length, d.reasons.length),
      `Epic's indications: the caller's words for a reason, as a code. ${NOT_USED} A booking sends the caller's own words today.`,
      this.add("a reason", d.reasons, () => ({ system: "" as ReasonSystem, code: "", words: [], visit_type: "", specialty: "" })),
    );
    out.push(this.listBlock("Diagnosis list", "diagnosis_list", "The hospital's own diagnosis list, for a reason whose code system is the hospital's diagnoses. Epic: when the hospital's code and a SNOMED code are both sent, the hospital's code wins."));
    const t = table("Reasons as codes", ["The caller's words", { head: "Code system", kind: "choice" }, { head: "Code", kind: "id" }, "Visit type", "Specialty"], LIST);
    this.fill(t, "reasons", rows, (r) => {
      const code = r.system === "hospital" ? this.idCell("diagnosis_list", r.code, (v) => (r.code = v)) : this.text("Code", r.code, (v) => (r.code = v));
      return row(
          r,
          r.words[0] || "A new reason",
          [
            this.wordsCell("The caller's words", r.words, (v) => (r.words = v)),
            this.choose(REASON_SYSTEMS, r.system, (v) => {
              r.system = v as ReasonSystem;
              this.kit.changed();
              this.kit.rerender();
            }),
            code,
            this.text("Visit type", r.visit_type, (v) => (r.visit_type = v)),
            this.text("Specialty", r.specialty, (v) => (r.specialty = v)),
          ],
          [
            ["Code system:", "SNOMED CT, ICD-10-CM, or the hospital's own diagnoses (the Diagnosis list above)."],
            ["Code:", "Epic's own sample sends SNOMED CT 46866001, a broken leg."],
          ],
          this.moves(d.reasons, r, "this reason"),
      );
    });
    out.push(t.wrap);
    return out;
  }

  timeZone(): HTMLElement[] {
    const d = this.d;
    const out = titleLine("Time zone", null, "The hospital's clock: the call's today and time, and each slot's time unless Epic sends the department's own zone.");
    const all = (Intl as unknown as { supportedValuesOf?: (k: string) => string[] }).supportedValuesOf?.("timeZone") ?? [];
    const zones = all.includes(d.time_zone) ? all : [d.time_zone, ...all];
    const t = table("Time zone", ["Zone"]);
    t.body.append(
      ...row(
        null,
        "Time zone",
        [this.choose(zones.map((z) => [z, z.replace(/_/g, " ")]), d.time_zone, (v) => (d.time_zone = v))],
        [["Zone:", "An IANA name. One zone for the whole hospital for now."]],
      ),
    );
    out.push(t.wrap);
    return out;
  }

  timesOfDay(): HTMLElement[] {
    const d = this.d;
    const rows = this.visible(d.times_of_day, (r) => textOf(r, String(r.hours)));
    const out = titleLine(
      "Times of day",
      this.count(rows.length, d.times_of_day.length),
      "The names a caller may use for a time of day, such as morning. The agent may choose only these names; Epic's $find gets the start and the length.",
      this.add("a time of day", d.times_of_day, () => ({ name: "", from: "08:00", hours: 1 }), this.l.times),
    );
    const t = table("Times of day", ["Name", { head: "Starts (HH:MM)", kind: "short" }, { head: "Hours", kind: "short" }], LIST);
    this.fill(t, "times-of-day", rows, (r) =>
      row(
        r,
        r.name || "A new time of day",
        [
          this.text("Name", r.name, (v) => (r.name = v)),
          this.text("Starts", r.from, (v) => (r.from = v)),
          this.num("Hours", r.hours, 1, 24, (v) => (r.hours = v)),
        ],
        [["Name:", "Lower case a to z, spaces and -. The model sees these names."]],
        this.moves(d.times_of_day, r, "this time of day"),
      ),
    );
    out.push(t.wrap);
    return out;
  }

  hours(): HTMLElement[] {
    const d = this.d;
    const dayName = (day: string) => DAYS.find(([k]) => k === day)?.[1] ?? "";
    const rows = this.visible(d.hours, (r) => textOf(r, dayName(r.day)));
    const out = titleLine(
      "Hours",
      this.count(rows.length, d.hours.length),
      "When the hospital is open, by day of the week. The agent says them when a caller asks when a place is open and the place has no hours of its own (where to go). Epic's $find returns open slots only.",
      this.add("a day", d.hours, () => ({ day: "mon", opens: "08:00", closes: "17:00" })),
    );
    const t = table("Hours", [{ head: "Day", kind: "choice" }, { head: "Opens (HH:MM)", kind: "short" }, { head: "Closes (HH:MM)", kind: "short" }], LIST);
    this.fill(t, "hours", rows, (r) =>
      row(
        r,
        dayName(r.day) || "A day",
        [
          this.choose(DAYS, r.day, (v) => (r.day = v)),
          this.text("Opens", r.opens, (v) => (r.opens = v)),
          this.text("Closes", r.closes, (v) => (r.closes = v)),
        ],
        [["Later:", "What the agent says when no person can take a hand-off now, and when a nurse may join a call."]],
        this.moves(d.hours, r, "this day"),
      ),
    );
    out.push(t.wrap);
    return out;
  }

  closedDays(): HTMLElement[] {
    const d = this.d;
    const rows = this.visible(d.closed_days);
    const out = titleLine(
      "Closed days",
      this.count(rows.length, d.closed_days.length),
      "Holidays and other days the hospital is closed. The next three from today go to the agent with a place's hours; it says one only when the caller asks about that day.",
      this.add("a closed day", d.closed_days, () => ({ date: "", name: "" })),
    );
    const t = table("Closed days", [{ head: "Date (YYYY-MM-DD)", kind: "short" }, "Name"], LIST);
    this.fill(t, "closed-days", rows, (r) =>
      row(
        r,
        r.name || r.date || "A new day",
        [this.text("Date", r.date, (v) => (r.date = v)), this.text("Name", r.name, (v) => (r.name = v))],
        [["Date:", "The day, in the hospital's time zone."]],
        this.moves(d.closed_days, r, "this closed day"),
      ),
    );
    out.push(t.wrap);
    return out;
  }

  // ---- Places › Locations (HOSPITAL-CONFIG.md §8.2): facilities as cards (1A), a card opens
  // by drilling in (2A), or every department in one table grouped by facility.

  /** The departments of a facility, or of none ("none"), in the list's order. */
  private departments(f: FacilityRow | "none"): LocationRow[] {
    // Grouped once per draw, not scanned once per facility (TABLES.md §6.1).
    if (!this.byFacility) {
      const ids = new Set(this.d.facilities.map((x) => x.id));
      const by = new Map<string, LocationRow[]>();
      const none: LocationRow[] = [];
      for (const l of this.d.locations) {
        if (!l.facility || !ids.has(l.facility)) none.push(l);
        else {
          const list = by.get(l.facility);
          if (list) list.push(l);
          else by.set(l.facility, [l]);
        }
      }
      this.byFacility = { by, none };
    }
    if (f === "none") return this.byFacility.none;
    return f.id.trim() ? (this.byFacility.by.get(f.id) ?? []) : [];
  }

  private byFacility?: { by: Map<string, LocationRow[]>; none: LocationRow[] };

  /** A specialty's spoken name by its ID ("ID 9" when it has none). */
  private specialtyName = (code: string): string => {
    if (!this.specialtyNames) this.specialtyNames = new Map(this.d.specialties.map((s) => [s.code, s.spoken]));
    return this.specialtyNames.get(code) || `ID ${code}`;
  };

  private specialtyNames?: Map<string, string>;

  /** A department's words for the search: its own, its specialty's name and its hours. */
  private departmentText = (l: LocationRow): string => textOf(l, l.specialty ? this.specialtyName(l.specialty) : "", this.hoursText(l.hours));

  /** A facility's own words match the search (its name, code, address, phone). */
  private facilityMatches(f: FacilityRow | "none"): boolean {
    return f !== "none" && this.ui.search.on && this.shows(f);
  }

  /** The departments of a facility that show: all when the facility itself matches. */
  private departmentsShown(f: FacilityRow | "none"): LocationRow[] {
    const all = this.departments(f);
    if (!this.ui.search.on || this.facilityMatches(f)) return all;
    return all.filter((l) => this.shows(l, () => this.departmentText(l)));
  }

  /** A facility shows when its own words or any of its departments match. */
  private facilityShows(f: FacilityRow | "none"): boolean {
    if (f === "none") return this.departmentsShown("none").length > 0;
    return !this.ui.search.on || this.facilityMatches(f) || this.departmentsShown(f).length > 0;
  }

  /** A row's import mark, as a word after its name: new, changed, or not in Epic. */
  private mark(row: object): HTMLElement | null {
    const m = this.ui.marks.get(row);
    if (!m) return null;
    const s = el(`<span class="place-mark"></span>`);
    s.dataset.mark = m;
    s.textContent = m === "new" ? "New" : m === "changed" ? "Changed" : "Not in Epic";
    return s;
  }

  /** A cell with the record's name and, after it, the row's import mark. */
  private nameCell(label: string, row: { record: string }): HTMLElement {
    const wrap = el(`<div class="sys-cell dt-cell"></div>`);
    wrap.append(this.text(label, row.record, (v) => (row.record = v)));
    const m = this.mark(row);
    if (m) wrap.append(m);
    return wrap;
  }

  /** One element of the stage's wave: it rises and fades in a step after the one before. */
  private wave<E extends HTMLElement>(e: E, i: number): E {
    e.dataset.wave = "";
    e.style.setProperty("--wave-i", String(Math.min(i, WAVE_LAST)));
    return e;
  }

  /** "Mon–Fri 08:00–17:00; Sat 09:00–12:00": a department's hours, read only. */
  private hoursText(hours: HoursRow[]): string {
    if (!hours.length) return "";
    const name = (day: string) => DAYS.find(([k]) => k === day)?.[1].slice(0, 3) ?? day;
    const spans: { from: string; to: string; time: string }[] = [];
    for (const h of hours) {
      const time = `${h.opens}–${h.closes}`;
      const last = spans[spans.length - 1];
      const next = DAYS.findIndex(([k]) => k === h.day);
      if (last && last.time === time && DAYS.findIndex(([k]) => k === last.to) === next - 1) last.to = h.day;
      else spans.push({ from: h.day, to: h.day, time });
    }
    return spans.map((s) => `${name(s.from)}${s.to !== s.from ? `–${name(s.to)}` : ""} ${s.time}`).join("; ");
  }

  /** A department's specialty, chosen from the Specialties list by its ID. */
  private specialtyCell(r: LocationRow): HTMLElement {
    this.specialtyOptions ??= [["", "None"], ...this.d.specialties.filter((s) => s.code.trim()).map((s): [string, string] => [s.code, s.spoken || s.code])];
    const options = [...this.specialtyOptions];
    if (r.specialty && !options.some(([c]) => c === r.specialty)) options.push([r.specialty, `ID ${r.specialty}`]);
    return this.choose(options, r.specialty, (v) => (r.specialty = v));
  }

  /** The Specialty select's options, made once per draw. */
  private specialtyOptions?: [string, string][];

  /** ↑ ↓ within its facility, and ✕ (Edit only). */
  private departmentMoves(r: LocationRow): HTMLElement {
    const same = (x: LocationRow) => (x.facility || "") === (r.facility || "");
    return listTools(this.d.locations, r, "this department", this.done, { peers: same, still: this.ui.search.on });
  }

  private static readonly DEPARTMENT_COLUMNS: Column[] = [
    { head: "The record's name" },
    { head: "Spoken name" },
    { head: "DEP ID", kind: "id" },
    { head: "Specialty", kind: "choice" },
    { head: "FHIR id" },
    { head: "Address" },
    { head: "Phone" },
    { head: "Hours" },
  ];

  /** One department's row: the same cells in the drill-in and the Table view. */
  private departmentRow(r: LocationRow): HTMLTableRowElement[] {
    // The prefix shows only beside an ID (the owner's screenshot, 2026-11-15: "R DEP" alone).
    const id = el(`<div class="sys-id dt-cell"><span class="sys-id-file"></span></div>`);
    const prefix = id.firstElementChild as HTMLElement;
    prefix.textContent = r.code.trim() ? "R DEP" : "";
    id.append(
      this.text("DEP ID", r.code, (v) => {
        r.code = v;
        prefix.textContent = v.trim() ? "R DEP" : "";
      }),
    );
    const hours = el(`<span class="place-hours"></span>`);
    hours.textContent = this.hoursText(r.hours);
    return row(
      r,
      r.record || "A new department",
      [
        this.nameCell("The record's name", r),
        this.text("Spoken name", r.spoken, (v) => (r.spoken = v)),
        id,
        this.specialtyCell(r),
        this.text("FHIR id", r.id, (v) => (r.id = v)),
        this.text("Address", r.address, (v) => (r.address = v)),
        this.text("Phone", r.phone, (v) => (r.phone = v)),
        hours,
      ],
      [
        ["The record's name:", "The department's name as the record holds it, such as EMC Family Medicine. A visit there says the spoken name."],
        ["Spoken name:", "What the caller hears. Empty: the record's own words."],
        ["DEP ID:", "The department's record ID in Epic (DEP)."],
        ["FHIR id:", "Epic's id for the Location: a visit's place is found by it first. Later: $find's location-reference."],
        ["Hours:", "From Epic's directory. The agent says them when a caller asks when this department is open; with none, the hospital's hours."],
      ],
      this.departmentMoves(r),
    );
  }

  /** Add a department to a facility (Edit only). */
  private addDepartment(facility: string): HTMLElement {
    const blank = (): LocationRow => ({ record: "", spoken: "", id: "", address: "", phone: "", facility, code: "", specialty: "", hours: [] });
    return this.add("a department", this.d.locations, blank, this.l.places);
  }

  /** The cards: one per facility, then the departments in none. */
  private cards(): HTMLElement[] {
    const d = this.d;
    const grid = el(`<div class="place-cards" role="list" aria-label="Facilities"></div>`);
    const specialtyName = this.specialtyName;
    const card = (f: FacilityRow | "none", i: number) => {
      const all = this.departments(f);
      // While a search is on, a facility shown for its departments counts and names only those.
      const depts = this.facilityMatches(f) ? all : this.departmentsShown(f);
      const c = el(`
        <button class="people-card place-card" type="button" role="listitem">
          <span class="people-band place-band"></span>
          <span class="people-body">
            <span class="people-line people-name"><span class="people-text"></span></span>
            <span class="people-line people-role place-code"><span class="people-text"></span></span>
            <span class="people-line people-role place-address"><span class="people-text"></span></span>
            <span class="people-line people-role place-count"><span class="people-text"></span></span>
            <span class="place-chips"></span>
          </span>
        </button>`);
      const [name, code, address, count] = [...c.querySelectorAll<HTMLElement>(".people-text")];
      name.textContent = f === "none" ? "Not in a facility" : f.spoken || f.record || "A new facility";
      if (f === "none" || !f.code) code.parentElement!.remove();
      else code.textContent = `R EAF ${f.code}`;
      if (f === "none" || !f.address) {
        address.textContent = f === "none" ? "Departments Epic names no facility for" : "Address";
        address.parentElement!.classList.add("is-placeholder");
      } else address.textContent = f.address;
      count.textContent =
        depts.length === all.length
          ? `${all.length} ${all.length === 1 ? "department" : "departments"}`
          : `${depts.length} of ${all.length} departments match`;
      const codes = [...new Set(depts.map((l) => l.specialty).filter(Boolean))];
      const chips = c.querySelector<HTMLElement>(".place-chips")!;
      for (const code of codes.slice(0, CHIPS)) {
        const chip = el(`<span class="place-chip"></span>`);
        chip.textContent = specialtyName(code);
        chips.append(chip);
      }
      if (codes.length > CHIPS) {
        const more = el(`<span class="place-chip place-chip-more"></span>`);
        more.textContent = `${codes.length - CHIPS} more`;
        chips.append(more);
      }
      if (!codes.length) chips.remove();
      const m = f !== "none" ? this.ui.marks.get(f) : undefined;
      if (m) c.dataset.mark = m;
      // An older version: a facility whose rows differ from the active version's.
      if (this.ui.older && this.facilityChanged(f)) c.classList.add("place-card-changed");
      c.setAttribute("aria-label", `${name.textContent}, ${count.textContent}`);
      c.addEventListener("click", () => this.ui.go(f === "none" ? "none" : facilityKey(f)));
      track(f === "none" ? NONE_GROUP : f, c);
      return this.wave(c, i);
    };
    const shown: (FacilityRow | "none")[] = d.facilities.filter((f) => this.facilityShows(f));
    if (this.facilityShows("none")) shown.push("none");
    const units: Unit[] = shown.map((f, i) => ({ item: f === "none" ? NONE_GROUP : f, draw: () => [card(f, i)] }));
    fillGrid(grid, units, { heights: heightsOf("facility-cards"), drawn: this.drawnLater });
    if (!units.length) {
      const empty = el(`<p class="empty"></p>`);
      empty.textContent = this.ui.search.on ? "No facility or department matches" : "No facilities yet: import them from Epic, or add one";
      grid.append(empty);
    }
    return [grid];
  }

  /** Whether a facility or its departments differ from the active version's (an older version
   * is shown). Matched by FHIR id, else the record's name. */
  private facilityChanged(f: FacilityRow | "none"): boolean {
    const a = this.ui.older!;
    const same = (x: { id: string; record: string }, y: { id: string; record: string }) =>
      x.id ? x.id === y.id : !y.id && x.record.trim().toLowerCase() === y.record.trim().toLowerCase();
    const theirs = f === "none" ? undefined : a.facilities.find((x) => same(f, x));
    if (f !== "none" && JSON.stringify(f) !== JSON.stringify(theirs)) return true;
    const mine = this.departments(f);
    const others = a.locations.filter((l) => (f === "none" ? !l.facility || !a.facilities.some((x) => x.id === l.facility) : !!f.id && l.facility === f.id));
    return mine.length !== others.length || mine.some((l) => JSON.stringify(l) !== JSON.stringify(others.find((o) => same(l, o))));
  }

  /** One facility, drilled in: back, its own fields, then its departments. */
  private facility(f: FacilityRow | "none"): HTMLElement[] {
    const d = this.d;
    let i = 0;
    const back = el(`<button class="cfg-link place-back" type="button">← All facilities</button>`);
    back.addEventListener("click", () => this.ui.go(null));
    const out: HTMLElement[] = [this.wave(back, i++)];
    const title = el(`<h4 class="place-title"><span></span><span class="sys-id-file"></span></h4>`);
    title.firstElementChild!.textContent = f === "none" ? "Not in a facility" : f.spoken || f.record || "A new facility";
    title.lastElementChild!.textContent = f !== "none" && f.code ? `R EAF ${f.code}` : "";
    out.push(this.wave(title, i++));
    if (f !== "none") {
      const t = table("Facility", ["The record's name", "Spoken name", { head: "EAF ID", kind: "id" }, "FHIR id", "Address", "Phone"], { remove: true });
      // ✕ takes the facility and its departments (the owner's import-all: analysts remove what
      // the agent does not need), then goes back to the cards (TABLES.md §3.11).
      const remove = rowTools("this facility and its departments", {
        remove: () => {
          const gone = new Set(this.departments(f));
          d.locations = d.locations.filter((l) => !gone.has(l));
          d.facilities.splice(d.facilities.indexOf(f), 1);
          this.kit.changed();
          this.ui.go(null);
        },
      });
      const [main, help] = row(
        f,
        f.record || "A new facility",
        [
          this.nameCell("The record's name", f),
          this.text("Spoken name", f.spoken, (v) => (f.spoken = v)),
          this.text("EAF ID", f.code, (v) => (f.code = v)),
          this.text("FHIR id", f.id, (v) => (f.id = v)),
          this.text("Address", f.address, (v) => (f.address = v)),
          this.text("Phone", f.phone, (v) => (f.phone = v)),
        ],
        [
          ["The record's name:", "The facility's name in Epic (EAF): a building or campus that holds departments."],
          ["Spoken name:", "What the caller hears for the facility when they ask where it is, and words a caller may use for it when they name a place."],
          ["FHIR id:", "Epic's id for the Location. Its departments name it as their facility."],
          ["Remove (✕):", "Takes out the facility and every department in it. Nothing is saved until Save."],
        ],
        remove,
      );
      t.body.append(main, help);
      out.push(this.wave(t.wrap, i++));
    }
    const depts = this.departmentsShown(f);
    // Add on the title line; a facility with no FHIR id cannot hold departments yet.
    const canAdd = f === "none" || !!f.id.trim();
    const [line, note] = titleLine(
      "Departments",
      this.count(depts.length, this.departments(f).length),
      canAdd ? null : "Type the facility's FHIR id to add departments to it.",
      ...(canAdd ? [this.addDepartment(f === "none" ? "" : f.id)] : []),
    );
    out.push(this.wave(line, i++));
    if (note) out.push(note);
    const t = table("Departments", Parts.DEPARTMENT_COLUMNS, LIST);
    const first = i;
    this.fill(t, "departments", depts, (r) => {
      const [main, help] = this.departmentRow(r);
      return [this.wave(main, first + depts.indexOf(r)), help];
    });
    out.push(t.wrap);
    return out;
  }

  /** Every department in one table, a heading row before each facility's (the owner, 2026-11-15:
   * "Table of DEPs grouped into an EAF"). */
  private placesTable(): HTMLElement[] {
    const d = this.d;
    const t = table("Departments by facility", Parts.DEPARTMENT_COLUMNS, LIST);
    if (this.ui.search.on) t.wrap.dataset.empty = "No match";
    // Each facility's heading row, then its departments: one list of rows, windowed when long.
    const units: Unit[] = [];
    const group = (f: FacilityRow | "none") => {
      if (!this.facilityShows(f) || (f === "none" && !this.departments("none").length)) return;
      const all = this.departments(f);
      const depts = this.departmentsShown(f);
      const at = units.length;
      units.push({
        item: f === "none" ? NONE_GROUP : f,
        draw: () => {
          const [name, code, count] = [el(`<span></span>`), el(`<span class="sys-id-file"></span>`), el(`<span class="dt-count"></span>`)];
          name.textContent = f === "none" ? "Not in a facility" : f.spoken || f.record || "A new facility";
          code.textContent = f !== "none" && f.code ? `R EAF ${f.code}` : "";
          count.textContent = depts.length === all.length ? String(all.length) : `${depts.length} of ${all.length}`;
          return [this.wave(groupRow(t, name, " ", code, " ", count), at)];
        },
      });
      for (const r of depts) {
        const k = units.length;
        units.push({
          item: r,
          draw: () => {
            const [main, help] = this.departmentRow(r);
            return [this.wave(main, k), help];
          },
        });
      }
    };
    d.facilities.forEach(group);
    group("none");
    fillTable(t, units, { heights: heightsOf("places-table"), drawn: this.drawnLater });
    return [t.wrap];
  }

  /** Import from Epic, on the Locations title line left of Add (the owner, 2026-11-15: "Go" on
   * TABLES.md's follow list; before, right of the tabs, his screenshot), and his B: "can
   * dropdown a small panel with a cities/states box and explicit instruction to enter city, state
   * or just state"). Edit only: an import fills the draft. The panel takes up to 5 searches, each
   * checked as it is added (a known state; a city of letters), and suggests the states and the
   * cities already in the page's addresses. */
  importControl(): HTMLElement {
    const ui = this.ui;
    const wrap = el(`
      <div class="place-import">
        <div class="cfg-control dt-add"><div class="tabs" role="group"><button class="tab place-import-open" type="button" aria-haspopup="dialog" aria-expanded="false">Import from Epic</button></div></div>
        <div class="menu place-import-panel" role="dialog" aria-label="Import from Epic" hidden>
          <div class="place-import-entry">
            <button class="cfg-info" type="button" aria-expanded="false" aria-label="How to search"><span class="i-glyph" aria-hidden="true">i</span></button>
          </div>
          <div class="cfg-help place-import-help"><p class="cfg-help-text">Enter a city and its state, such as Verona, WI, or just a state, such as WI or Wisconsin. Press Enter to add each one, up to ${PLACE_SEARCHES}. Epic sends every facility and department it finds there.</p></div>
          <div class="place-import-list" id="place-import-list" role="listbox" aria-label="Suggestions"></div>
          <p class="place-import-problem" role="alert" hidden></p>
          <div class="place-chips place-import-chips" aria-label="To search"></div>
          <div class="tabs cfg-actions place-import-actions" role="group" aria-label="Import">
            <button class="tab" type="button" data-act="cancel">Cancel</button>
            <button class="tab cfg-save" type="button" data-act="import" disabled>Import</button>
          </div>
          <p class="sys-note place-import-say" role="status" hidden></p>
        </div>
      </div>`);
    const open = wrap.querySelector<HTMLButtonElement>(".place-import-open")!;
    const panel = wrap.querySelector<HTMLElement>(".place-import-panel")!;
    // The Config tables' own text box (system.ts › field): its size, edge and rounding.
    const box = field("A city and its state, or a state", "", this.l.cell, () => {});
    box.classList.add("place-import-box");
    wrap.querySelector(".place-import-entry")!.append(box);
    const input = box.querySelector<HTMLTextAreaElement>("textarea")!;
    input.placeholder = "City, State";
    input.spellcheck = false;
    input.setAttribute("autocomplete", "off");
    input.setAttribute("aria-autocomplete", "list");
    input.setAttribute("aria-controls", "place-import-list");
    const info = wrap.querySelector<HTMLButtonElement>(".cfg-info")!;
    const help = wrap.querySelector<HTMLElement>(".place-import-help")!;
    // The help rolls down and up, as every "i" on Config does (config.css › .cfg-help).
    info.addEventListener("click", () => {
      const opened = !help.classList.contains("open");
      help.classList.toggle("open", opened);
      info.setAttribute("aria-expanded", String(opened));
    });
    const list = wrap.querySelector<HTMLElement>(".place-import-list")!;
    const problem = wrap.querySelector<HTMLElement>(".place-import-problem")!;
    const chips = wrap.querySelector<HTMLElement>(".place-import-chips")!;
    const say = wrap.querySelector<HTMLElement>(".place-import-say")!;
    const go = wrap.querySelector<HTMLButtonElement>('[data-act="import"]')!;
    open.title = "Edit only: an import fills the page, and Save keeps it";

    // The cities already in this hospital's addresses: after a first import, the next one is a
    // pick, not a typing job.
    const cities = citiesIn([...this.d.facilities, ...this.d.locations].map((r) => r.address));
    type Suggestion = { value: string; detail: string };
    let shown: Suggestion[] = [];
    let active = -1;

    const tell = (text: string) => {
      problem.textContent = text;
      problem.hidden = !text;
    };
    // Import greys, as Save does, while there is nothing to search.
    const ready = () => (go.disabled = !ui.importEntries.length && !input.value.trim());
    const drawChips = () => {
      chips.replaceChildren(
        ...ui.importEntries.map((entry) => {
          const chip = el(`<span class="place-chip"><span></span><button class="place-chip-x" type="button">×</button></span>`);
          chip.firstElementChild!.textContent = entry;
          const x = chip.querySelector("button")!;
          x.setAttribute("aria-label", `Remove ${entry}`);
          x.addEventListener("click", () => {
            ui.importEntries = ui.importEntries.filter((e) => e !== entry);
            drawChips();
            input.focus();
          });
          return chip;
        }),
      );
      chips.hidden = !ui.importEntries.length;
      ready();
    };
    const suggest = () => {
      const q = input.value.trim().toLowerCase();
      const out: Suggestion[] = [];
      const push = (s: Suggestion) => {
        if (out.length < SUGGESTIONS && !out.some((o) => o.value === s.value) && !ui.importEntries.includes(s.value)) out.push(s);
      };
      for (const c of cities) if (!q || c.toLowerCase().startsWith(q)) push({ value: c, detail: "In this hospital's addresses" });
      if (q) {
        for (const [code, name] of US_STATES) {
          if (code.toLowerCase().startsWith(q) || name.toLowerCase().startsWith(q)) push({ value: code, detail: name });
        }
        const typed = placeSearch(input.value);
        if (typed) push({ value: typed, detail: typed.includes(",") ? "A city and its state" : "A state" });
      }
      shown = out;
      active = out.length && q ? 0 : -1;
      list.replaceChildren(
        ...out.map((s, i) => {
          const r = el(`<div class="menu-row word-row" role="option"><span class="word-main"><span class="word-name"></span><span class="word-note"></span></span></div>`);
          r.querySelector(".word-name")!.textContent = s.value;
          r.querySelector(".word-note")!.textContent = s.detail;
          r.setAttribute("aria-selected", String(i === active));
          // A row keeps the box's focus: the choice is made on the press.
          r.addEventListener("mousedown", (e) => {
            e.preventDefault();
            add(s.value);
          });
          return r;
        }),
      );
      list.hidden = !out.length;
    };
    const mark = () => [...list.children].forEach((r, i) => r.setAttribute("aria-selected", String(i === active)));
    const add = (text: string): boolean => {
      const entry = placeSearch(text);
      if (!entry) {
        tell("That is not a state, or a city and its state. Try Verona, WI or WI.");
        return false;
      }
      if (ui.importEntries.includes(entry)) {
        tell(`${entry} is on the list already.`);
        return false;
      }
      if (ui.importEntries.length >= PLACE_SEARCHES) {
        tell(`Up to ${PLACE_SEARCHES} at a time.`);
        return false;
      }
      ui.importEntries = [...ui.importEntries, entry];
      input.value = "";
      tell("");
      drawChips();
      suggest();
      return true;
    };
    const outside = (e: MouseEvent) => {
      if (!wrap.isConnected) return document.removeEventListener("mousedown", outside, true);
      if (!wrap.contains(e.target as Node)) close();
    };
    const close = () => {
      panel.hidden = true;
      open.setAttribute("aria-expanded", "false");
      document.removeEventListener("mousedown", outside, true);
    };
    const show = () => {
      panel.hidden = false;
      open.setAttribute("aria-expanded", "true");
      drawChips();
      suggest();
      say.textContent = "";
      say.hidden = true;
      document.addEventListener("mousedown", outside, true);
      input.focus();
    };

    open.addEventListener("click", () => (panel.hidden ? show() : close()));
    input.addEventListener("input", () => {
      tell("");
      suggest();
      ready();
    });
    input.addEventListener("keydown", (e) => {
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        if (!shown.length) return;
        e.preventDefault();
        active = (active + (e.key === "ArrowDown" ? 1 : -1) + shown.length) % shown.length;
        mark();
      } else if (e.key === "Enter") {
        e.preventDefault();
        if (active >= 0 && shown[active]) add(shown[active].value);
        else if (input.value.trim()) add(input.value);
        else if (ui.importEntries.length) go.click();
      } else if (e.key === "Escape") {
        e.preventDefault();
        close();
        open.focus();
      } else if (e.key === "Backspace" && !input.value && ui.importEntries.length) {
        ui.importEntries = ui.importEntries.slice(0, -1);
        drawChips();
        suggest();
      }
    });
    panel.querySelector('[data-act="cancel"]')!.addEventListener("click", () => {
      close();
      open.focus();
    });
    go.addEventListener("click", () => {
      // Text typed but not added yet goes in first, so Import does what the box shows.
      if (input.value.trim() && !add(input.value)) return;
      if (!ui.importEntries.length) return tell("Add a city and its state, or a state, first.");
      ui.runImport(this.d, ui.importEntries, say, go);
    });
    return wrap;
  }

  places(): HTMLElement[] {
    const d = this.d;
    const matching = this.ui.search.on
      ? [...d.facilities, "none" as const].reduce((n, f) => n + (this.facilityShows(f) ? this.departmentsShown(f).length : 0), 0)
      : d.locations.length;
    const out = titleLine(
      "Locations",
      this.count(matching, d.locations.length),
      `${d.facilities.length} ${d.facilities.length === 1 ? "facility" : "facilities"} (Epic's EAF), each holding its departments (DEP). A visit at a department on this list says its spoken name; the first department is said when a record names no place.`,
      this.importControl(),
      // A new facility is a card with nothing to show: Add opens it, drilled in, to fill.
      addButton("Add a facility", () => {
        if (d.facilities.length >= this.l.places) return;
        const f: FacilityRow = { record: "", spoken: "", id: "", code: "", address: "", phone: "" };
        this.ui.search.keep(f);
        d.facilities.push(f);
        this.kit.changed();
        this.ui.openNew(f);
      }),
    );
    // What the last import did (counts only), until the next draw of a new draft.
    if (this.ui.importSay) {
      const said = el(`<p class="sys-note place-import-done" role="status"></p>`);
      said.textContent = this.ui.importSay;
      out.push(said);
    }
    const bar = tabBar(
      "Show the locations as",
      [
        ["cards", "Cards"],
        ["table", "Table"],
      ],
      this.ui.view,
      (v) => this.ui.setView(v as PlacesView),
    );
    bar.el.classList.add("place-toggle");
    this.ui.toggle = bar.el;
    requestAnimationFrame(() => bar.place());
    new ResizeObserver(() => bar.place()).observe(bar.el);
    out.push(bar.el);
    const stage = el(`<div class="place-stage"></div>`);
    const open = this.ui.view === "cards" ? resolveOpen(d, this.ui.open, this.ui.fresh) : null;
    stage.append(...(this.ui.view === "table" ? this.placesTable() : open ? this.facility(open) : this.cards()));
    if (this.ui.wave) {
      stage.classList.add("place-wave");
      // After the wave, rows a long list draws as it scrolls come in without it.
      window.setTimeout(() => stage.classList.remove("place-wave"), tokenMs("--dur-wave-in") + WAVE_LAST * tokenMs("--wave-step"));
    }
    this.ui.wave = false;
    this.ui.stage = stage;
    out.push(stage);
    return out;
  }

  specialties(): HTMLElement[] {
    const d = this.d;
    const rows = this.visible(d.specialties);
    const out = titleLine(
      "Specialties",
      this.count(rows.length, d.specialties.length),
      `Epic's specialties: a kind of care, such as Cardiology. Each department on Places names one by its ID; here each ID gets the name the caller hears and the caller's words for it, such as "a skin doctor". Primary care: a caller with no visit with their PCP has their times searched at their last department of a primary care specialty, and a call that names no specialty searches the primary care departments. A caller's words for a kind of care are matched to the spoken name and the caller's words here; a specialty with no ID matches no department.`,
      this.add("a specialty", d.specialties, () => ({ code: "", spoken: "", words: [], primary_care: false })),
    );
    out.push(this.listBlock("Specialty list", "specialty_list", "The list every specialty's ID belongs to. Epic gives no list of a hospital's specialties by API."));
    const t = table("Specialties", ["Spoken name", "The caller's words", { head: "ID", kind: "id" }, { head: "Primary care", kind: "flag" }], LIST);
    this.fill(t, "specialties", rows, (r) =>
      row(
        r,
        r.spoken || "A new specialty",
        [
          this.text("Spoken name", r.spoken, (v) => (r.spoken = v)),
          this.wordsCell("The caller's words", r.words, (v) => (r.words = v)),
          this.idCell("specialty_list", r.code, (v) => (r.code = v)),
          this.onOff("Primary care", r.primary_care, (v) => (r.primary_care = v)),
        ],
        [
          ["ID:", "The specialty's ID in the specialty list, from the hospital's own build."],
          ["Primary care:", "On: the caller's last department of this specialty is their usual place for a visit with no specialty named, when they have no visit with their PCP."],
        ],
        this.moves(d.specialties, r, "this specialty"),
      ),
    );
    out.push(t.wrap);
    return out;
  }

  booking(): HTMLElement[] {
    const d = this.d;
    const out = titleLine("Booking", null, "How the agent offers slots, finds a caller's visits, and notes a booking.");
    const t = table("Booking", ["Setting", "Value"]);
    const windows = field("Search windows (days, comma separated)", d.ahead_windows.join(", "), 40, (v) => {
      d.ahead_windows = unwords(v).map(Number);
      this.edited();
    });
    const rows: [string, HTMLElement, string][] = [
      ["Slots offered at a time", this.num("Slots offered at a time", d.slots_offered, 1, 5, (v) => (d.slots_offered = v)), "How many open slots the agent offers in one turn. The caller can ask for more."],
      ["Visits before the narrowing question", this.num("Visits before the narrowing question", d.narrow_over, 1, 10, (v) => (d.narrow_over = v)), "More visits than this match what the caller named, and the agent asks which group they mean."],
      ["Groups one question names", this.num("Groups one question names", d.groups_named, 1, 5, (v) => (d.groups_named = v)), "The rest are \"another visit\"."],
      ["Days back for a past visit", this.num("Days back for a past visit", d.past_days, 1, 3650, (v) => (d.past_days = v)), "How far back \"when was I last seen?\" looks."],
      [
        "Years for an established patient",
        this.num("Years for an established patient", d.established_years, 1, 10, (v) => (d.established_years = v)),
        "A caller seen by a specialty within this many years is established in it; else new (the CPT rule's 3 years). Read only when a visit type is for new or established patients: one search of the caller's visits a call.",
      ],
      [
        "Offer a sooner time with another clinician",
        this.onOff("Offer a sooner time with another clinician", d.sooner_with_others, (v) => (d.sooner_with_others = v)),
        "With the caller's PCP's times first, also offer one time with another clinician at the same place when it is on an earlier day than the PCP's first. Off: the PCP's times, then the others, with none called sooner.",
      ],
      ["Search windows (days)", windows, "The search for the visits to come widens through these windows, then has no end. Each window is longer than the one before. One read to Epic per window."],
      [
        "Label after the reason in the booking note",
        this.text("Label after the reason in the booking note", d.booking_note_label, (v) => (d.booking_note_label = v)),
        "Epic's $book files the caller's reason, in their own words, as the visit's note; this label follows it, so staff know who said it. Empty: the words alone. Epic keeps 500 characters; the label is kept whole.",
      ],
    ];
    // A search shows the settings whose name or help holds its words.
    if (this.ui.search.on) t.wrap.dataset.empty = "No match";
    for (const [name, control, help] of rows) {
      if (this.ui.search.on && !this.ui.search.matches(`${name}\n${help}`)) continue;
      t.body.append(...row(null, name, [name, control], [["What it does:", help]]));
    }
    out.push(t.wrap);
    return out;
  }

  /** The Booking rules (BOOKING-RULES.md §3.5; the owner's E-A, the rules engine's start). The
   * first row that matches wins, so the order is kept and ↑ ↓ move a row. */
  rules(): HTMLElement[] {
    const d = this.d;
    const rows = this.visible(d.booking_rules, (r) => textOf(r, RULE_OUTCOMES.find(([o]) => o === r.outcome)?.[1] ?? ""));
    const out = titleLine(
      "Booking rules",
      this.count(rows.length, d.booking_rules.length),
      "What the agent does with a visit, by its specialty, department, visit type, provider, and whether the caller is new or established there. Each condition left empty is Any. The first row that matches wins; a visit no row matches is booked. Specialty, department and new or established are known before the search, so a rule on them alone can answer with no search; a visit type or a provider is checked on each time. The agent never checks whether a caller has a referral: the rule says what happens.",
      this.add("a rule", d.booking_rules, () => ({ specialty: "", department: "", visit_type: "", provider: "", patients: "either", outcome: "book" })),
    );
    const specialties: [string, string][] = [["", "Any"], ...d.specialties.filter((s) => s.code.trim()).map((s): [string, string] => [s.code, s.spoken || s.code])];
    const types: [string, string][] = [["", "Any"], ...d.visit_types.filter((v) => v.record.trim()).map((v): [string, string] => [v.record, v.record])];
    const keep = (options: [string, string][], value: string): [string, string][] =>
      value && !options.some(([c]) => c === value) ? [...options, [value, `${value} (not on the list)`]] : options;
    const t = table(
      "Booking rules",
      [
        { head: "Specialty", kind: "choice" },
        "Department",
        { head: "Visit type", kind: "choice" },
        "Provider",
        { head: "Patients", kind: "choice" },
        { head: "Outcome", kind: "choice" },
      ],
      LIST,
    );
    this.fill(t, "booking-rules", rows, (r) =>
      row(
        r,
        `Rule ${d.booking_rules.indexOf(r) + 1}`,
        [
          this.choose(keep(specialties, r.specialty), r.specialty, (v) => (r.specialty = v)),
          this.text("Department", r.department, (v) => (r.department = v.trim())),
          this.choose(keep(types, r.visit_type), r.visit_type, (v) => (r.visit_type = v)),
          this.text("Provider", r.provider, (v) => (r.provider = v.trim())),
          this.choose(RULE_PATIENTS, r.patients, (v) => (r.patients = v as PatientKind)),
          this.choose(RULE_OUTCOMES, r.outcome, (v) => (r.outcome = v as RuleOutcome)),
        ],
        [
          ["Department:", "A department's record name, as on Places (any case). Empty: any department."],
          ["Provider:", "The provider's name as the record shows it, such as Dr. Rosa Patel; a title is left out when it is compared. Empty: any provider."],
          ["Patients:", "New: not seen by the department's specialty within Booking › Years for an established patient. Established: seen by it. Any: either."],
          ["Outcome:", RULE_HELP[r.outcome]],
        ],
        this.moves(d.booking_rules, r, "this rule"),
      ),
    );
    out.push(t.wrap);
    return out;
  }
}
