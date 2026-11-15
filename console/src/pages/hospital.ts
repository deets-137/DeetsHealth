// Config › Hospital (HOSPITAL-CONFIG.md; the owner, 2026-11-15: every section now, so the page
// shows what is left to build; one hospital for every EHR source). The hospital's own lists and
// numbers, in the Config frame (Read | Edit, the versions, Discard | Save), as tabs of tables
// that are edited in place (System's parts: system.ts). A save makes a new active version; each
// call takes the active version when it opens, and keeps it to its end.
//
// Some lists are kept but read by no call yet: each one's note says so ("Not used by calls
// yet"). No row is protected data: a row is a fact about the hospital. Every size, color and
// timing is an existing token: the page reuses System's classes and adds none.
import { saveHospital, type HospitalSettings, type HospitalValues, type ReasonSystem } from "../agent/api";
import { hospitalSettings } from "../agent/stores";
import { settingsView, type Kit } from "./config-kit";
import { describeEpicOid, recordPrefix } from "./epic-oid";
import { el } from "./page";
import { field, grow, heading, row, table } from "./system";
import type { View } from "./subnav";
import { remember, remembered, tabBar } from "./tab-bar";

type Draft = HospitalValues;
type Key = keyof HospitalValues;
type ListKey = "visit_type_list" | "diagnosis_list" | "specialty_list";
type Limits = HospitalSettings["limits"];
const TABS = ["visits", "times", "places", "booking"] as const;
type Tab = (typeof TABS)[number];
const TAB_KEY = "deets.console.hospital.tab";
/** Each tab's lists, in the page's order. */
const TAB_KEYS: Record<Tab, Key[]> = {
  visits: ["visit_type_list", "visit_types", "visit_groups", "diagnosis_list", "reasons"],
  times: ["time_zone", "times_of_day", "hours", "closed_days"],
  places: ["locations", "specialty_list", "specialties"],
  booking: ["slots_offered", "narrow_over", "groups_named", "past_days", "ahead_windows", "booking_note_label"],
};
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

/** What does not fit, in the page's words, or null: the rules of ehr's hospital.rs, which
 * checks again on save. Names a list and a row, never a value. */
function problem(d: Draft, l: Limits): string | null {
  const one = (text: string, needed: boolean) =>
    (needed && !text.trim()) || text.length > l.cell || /[\u0000-\u001f\u007f-\u009f]/.test(text);
  const many = (list: string[]) => list.length > l.words || list.some((w) => one(w, true));
  const each = <T>(name: string, rows: T[], check: (r: T) => string | null): string | null => {
    if (rows.length > l.rows) return `${name}: at most ${l.rows} rows`;
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
      each("Locations", d.locations, (r) =>
        one(r.record, true) ? "the record's name is empty or too long" : [r.spoken, r.id, r.address, r.phone].some((x) => one(x, false)) ? "a cell is too long" : null,
      ),
    () => (twice(d.locations, (r) => r.record) ? `Locations, row ${twice(d.locations, (r) => r.record)}: the record's name is there twice` : null),
    () =>
      each("Specialties", d.specialties, (s) =>
        one(s.spoken, true) ? "the spoken name is empty or too long" : many(s.words) ? "too many words, or one too long" : one(s.code, false) ? "a cell is too long" : null,
      ),
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

function differing(a: Draft, b: Draft): Set<string> {
  const out = new Set<string>();
  for (const k of Object.keys({ ...a, ...b }) as Key[]) if (JSON.stringify(a[k]) !== JSON.stringify(b[k])) out.add(k);
  return out;
}

export const hospitalView = (): View => {
  let tab: Tab = remembered(TAB_KEY, TABS, "visits");
  let slideTab: string | undefined;
  let latest: HospitalSettings | undefined;

  return settingsView<HospitalValues, Draft, HospitalSettings>({
    id: "hospital",
    title: "Hospital",
    store: hospitalSettings,
    took: (s) => (latest = s),
    save: (_s, d) => saveHospital(d),
    fromValues: (v) => structuredClone(v),
    differing,
    blocked: (d) => (latest ? problem(d, latest.limits) : null),
    render: (s, d, kit) => render(s, d, kit),
  });

  function render(s: HospitalSettings, d: Draft, kit: Kit) {
    const card = el(`<section class="cfg-card prompt-card"></section>`);
    card.dataset.card = "hospital";
    const from = slideTab;
    slideTab = undefined;
    const bar = tabBar(
      "Hospital",
      [
        ["visits", "Visits"],
        ["times", "Times"],
        ["places", "Places"],
        ["booking", "Booking"],
      ],
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
    bars.append(bar.el);
    const body = el(`<div class="prompt-sections"></div>`);
    const p = new Parts(s.limits, d, kit);
    if (tab === "visits") body.append(...p.visitTypes(), ...p.visitGroups(), ...p.reasons());
    else if (tab === "times") body.append(...p.timeZone(), ...p.timesOfDay(), ...p.hours(), ...p.closedDays());
    else if (tab === "places") body.append(...p.locations(), ...p.specialties());
    else body.append(...p.booking());
    const err = el(`<p class="prompt-problem" role="status" hidden></p>`);
    const showProblem = () => {
      const msg = problem(d, s.limits);
      err.textContent = msg ?? "";
      err.hidden = !msg;
    };
    p.onEdit = showProblem;
    showProblem();
    card.append(bars, err, body);
    const settle = () => {
      card.querySelectorAll<HTMLTextAreaElement>("textarea").forEach(grow);
      bar.place();
    };
    requestAnimationFrame(settle);
    new ResizeObserver(settle).observe(card);
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
  ) {}

  private edited() {
    this.kit.changed();
    this.onEdit();
  }

  /** A one-line text cell. */
  private text(label: string, value: string, on: (v: string) => void): HTMLElement {
    return field(label, value, this.l.cell, (v) => (on(v), this.edited()));
  }

  /** A cell of words, comma separated. */
  private wordsCell(label: string, value: string[], on: (v: string[]) => void): HTMLElement {
    return field(`${label} (comma separated)`, words(value), this.l.cell * 4, (v) => (on(unwords(v)), this.edited()));
  }

  /** Off | On in a cell. */
  private onOff(label: string, value: boolean, on: (v: boolean) => void): HTMLElement {
    const wrap = el(`<div class="cfg-control"></div>`);
    wrap.append(this.kit.pair(value, (v) => (on(v), this.onEdit()), label));
    return wrap;
  }

  private choose(options: [string, string][], value: string, on: (v: string) => void): HTMLElement {
    const wrap = el(`<div class="cfg-control"></div>`);
    wrap.append(this.kit.select(options, value, (v) => (on(v), this.onEdit())));
    return wrap;
  }

  private num(label: string, value: number, min: number, max: number, on: (v: number) => void): HTMLElement {
    const wrap = el(`<div class="cfg-control"></div>`);
    wrap.append(this.kit.number(value, min, max, (v) => (on(v), this.onEdit()), label));
    return wrap;
  }

  /** Up and Remove for one row (Edit only): the order matters where a note says so. */
  private moves<T>(rows: T[], r: T): HTMLElement {
    const wrap = el(`<div class="cfg-control sys-remove"><button class="cfg-link" type="button">Up</button><button class="cfg-link" type="button">Remove</button></div>`);
    const [up, remove] = [...wrap.querySelectorAll<HTMLButtonElement>("button")];
    const i = rows.indexOf(r);
    if (i === 0) {
      up.dataset.off = "";
      up.hidden = true;
    }
    up.addEventListener("click", () => {
      const at = rows.indexOf(r);
      if (at > 0) [rows[at - 1], rows[at]] = [rows[at], rows[at - 1]];
      this.kit.changed();
      this.kit.rerender();
    });
    remove.addEventListener("click", () => {
      rows.splice(rows.indexOf(r), 1);
      this.kit.changed();
      this.kit.rerender();
    });
    return wrap;
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
      ...row(title, [oid, says], [
        ["The OID:", help],
        ["Where it comes from:", "The hospital's Epic team runs Epic's OID Generator utility and sends it. It differs between the hospital's test and production Epic."],
      ]),
    );
    return t.wrap;
  }

  /** A row's ID in its list, after the list's prefix ("R PRC 1003"). The prefix is shown only;
   * the OID and the ID are what is kept and sent. */
  private idCell(key: ListKey, value: string, on: (v: string) => void): HTMLElement {
    const wrap = el(`<div class="sys-id"><span class="sys-id-file"></span></div>`);
    const prefix = wrap.firstElementChild as HTMLElement;
    prefix.textContent = recordPrefix(this.d[key]);
    this.prefixes.set(key, [...(this.prefixes.get(key) ?? []), prefix]);
    wrap.append(this.text("ID", value, on));
    return wrap;
  }

  /** Add a row (Edit only): an empty row at the end, to fill in place. */
  private add<T>(what: string, rows: T[], blank: () => T): HTMLElement {
    const wrap = el(`<div class="sys-add"><div class="cfg-control"><button class="cfg-button" type="button"></button></div></div>`);
    const b = wrap.querySelector<HTMLButtonElement>("button")!;
    b.textContent = `Add ${what}`;
    b.addEventListener("click", () => {
      if (rows.length >= this.l.rows) return;
      rows.push(blank());
      this.kit.changed();
      this.kit.rerender();
    });
    return wrap;
  }

  visitTypes(): HTMLElement[] {
    const d = this.d;
    const out = heading(
      "Visit types",
      d.visit_types.length,
      "Epic's service-type. A visit whose type is on this list says the spoken name and takes the Online mark. Epic searches each type the agent may book that has an ID; the local fake books the first of each kind, in this order.",
    );
    out.push(this.listBlock("Visit type list", "visit_type_list", "The list every visit type's ID belongs to, sent with each ID to Epic's $find."));
    const t = table("Visit types", ["The record's name", "Spoken name", "ID", "Online", "Agent may book", ""]);
    for (const r of d.visit_types) {
      t.body.append(
        ...row(
          r.record || "A new visit type",
          [
            this.text("The record's name", r.record, (v) => (r.record = v)),
            this.text("Spoken name", r.spoken, (v) => (r.spoken = v)),
            this.idCell("visit_type_list", r.code, (v) => (r.code = v)),
            this.onOff("Online", r.online, (v) => (r.online = v)),
            this.onOff("Agent may book", r.book, (v) => (r.book = v)),
            this.moves(d.visit_types, r),
          ],
          [
            ["The record's name:", "The type's name as the record holds it. A visit of this name, in any case, takes this row."],
            ["Spoken name:", "What the caller hears for this type. Empty: the record's own words. A name that names sensitive care makes the visit private."],
            ["ID:", r.code ? "The type's ID in the visit type list, sent to Epic's $find." : "None: Epic cannot search or book this type. The local fake can."],
          ],
        ),
      );
    }
    out.push(t.wrap, this.add("a visit type", d.visit_types, () => ({ code: "", record: "", spoken: "", online: false, book: false })));
    return out;
  }

  visitGroups(): HTMLElement[] {
    const d = this.d;
    const out = heading(
      "Visit groups",
      d.visit_groups.length,
      "When a caller has several visits, the agent asks which group they mean. A group whose name or words name sensitive care is never said.",
    );
    const t = table("Visit groups", ["Name the caller hears", "The caller's words", "Visit types", ""]);
    for (const g of d.visit_groups) {
      t.body.append(
        ...row(
          g.name || "A new group",
          [
            this.text("Name the caller hears", g.name, (v) => (g.name = v)),
            this.wordsCell("The caller's words", g.words, (v) => (g.words = v)),
            this.wordsCell("Visit types", g.visit_types, (v) => (g.visit_types = v)),
            this.moves(d.visit_groups, g),
          ],
          [
            ["The caller's words:", "Other words a caller may say for this group, such as \"my checkup\"."],
            ["Visit types:", "The members, by the record's name of each visit type."],
          ],
        ),
      );
    }
    out.push(t.wrap, this.add("a group", d.visit_groups, () => ({ name: "", words: [], visit_types: [] })));
    return out;
  }

  reasons(): HTMLElement[] {
    const d = this.d;
    const out = heading("Reasons as codes", d.reasons.length, `Epic's indications: the caller's words for a reason, as a code. ${NOT_USED} A booking sends the caller's own words today.`);
    out.push(this.listBlock("Diagnosis list", "diagnosis_list", "The hospital's own diagnosis list, for a reason whose code system is the hospital's diagnoses. Epic: when the hospital's code and a SNOMED code are both sent, the hospital's code wins."));
    const t = table("Reasons as codes", ["The caller's words", "Code system", "Code", "Visit type", "Specialty", ""]);
    for (const r of d.reasons) {
      const code = r.system === "hospital" ? this.idCell("diagnosis_list", r.code, (v) => (r.code = v)) : this.text("Code", r.code, (v) => (r.code = v));
      t.body.append(
        ...row(
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
            this.moves(d.reasons, r),
          ],
          [
            ["Code system:", "SNOMED CT, ICD-10-CM, or the hospital's own diagnoses (the Diagnosis list above)."],
            ["Code:", "Epic's own sample sends SNOMED CT 46866001, a broken leg."],
          ],
        ),
      );
    }
    out.push(t.wrap, this.add("a reason", d.reasons, () => ({ system: "" as ReasonSystem, code: "", words: [], visit_type: "", specialty: "" })));
    return out;
  }

  timeZone(): HTMLElement[] {
    const d = this.d;
    const out = heading("Time zone", 1, "The hospital's clock: the call's today and time, and each slot's time unless Epic sends the department's own zone.");
    const all = (Intl as unknown as { supportedValuesOf?: (k: string) => string[] }).supportedValuesOf?.("timeZone") ?? [];
    const zones = all.includes(d.time_zone) ? all : [d.time_zone, ...all];
    const t = table("Time zone", ["Zone"]);
    t.body.append(
      ...row(
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
    const out = heading(
      "Times of day",
      d.times_of_day.length,
      "The names a caller may use for a time of day, such as morning. The agent may choose only these names; Epic's $find gets the start and the length.",
    );
    const t = table("Times of day", ["Name", "Starts (HH:MM)", "Hours", ""]);
    for (const r of d.times_of_day) {
      t.body.append(
        ...row(
          r.name || "A new time of day",
          [
            this.text("Name", r.name, (v) => (r.name = v)),
            this.text("Starts", r.from, (v) => (r.from = v)),
            this.num("Hours", r.hours, 1, 24, (v) => (r.hours = v)),
            this.moves(d.times_of_day, r),
          ],
          [["Name:", "Lower case a to z, spaces and -. The model sees these names."]],
        ),
      );
    }
    out.push(t.wrap, this.add("a time of day", d.times_of_day, () => ({ name: "", from: "08:00", hours: 1 })));
    return out;
  }

  hours(): HTMLElement[] {
    const d = this.d;
    const out = heading("Hours", d.hours.length, `When the hospital is open, by day of the week. ${NOT_USED} Epic's $find returns open slots only.`);
    const t = table("Hours", ["Day", "Opens (HH:MM)", "Closes (HH:MM)", ""]);
    for (const r of d.hours) {
      t.body.append(
        ...row(
          DAYS.find(([k]) => k === r.day)?.[1] ?? "A day",
          [
            this.choose(DAYS, r.day, (v) => (r.day = v)),
            this.text("Opens", r.opens, (v) => (r.opens = v)),
            this.text("Closes", r.closes, (v) => (r.closes = v)),
            this.moves(d.hours, r),
          ],
          [["Later:", "What the agent says when no person can take a hand-off now, and when a nurse may join a call."]],
        ),
      );
    }
    out.push(t.wrap, this.add("a day", d.hours, () => ({ day: "mon", opens: "08:00", closes: "17:00" })));
    return out;
  }

  closedDays(): HTMLElement[] {
    const d = this.d;
    const out = heading("Closed days", d.closed_days.length, `Holidays and other days the hospital is closed. ${NOT_USED}`);
    const t = table("Closed days", ["Date (YYYY-MM-DD)", "Name", ""]);
    for (const r of d.closed_days) {
      t.body.append(
        ...row(
          r.name || r.date || "A new day",
          [this.text("Date", r.date, (v) => (r.date = v)), this.text("Name", r.name, (v) => (r.name = v)), this.moves(d.closed_days, r)],
          [["Date:", "The day, in the hospital's time zone."]],
        ),
      );
    }
    out.push(t.wrap, this.add("a closed day", d.closed_days, () => ({ date: "", name: "" })));
    return out;
  }

  locations(): HTMLElement[] {
    const d = this.d;
    const out = heading(
      "Locations",
      d.locations.length,
      "A visit at a place on this list says its spoken name. The first row is said when a record names no place. The FHIR id, the address and the phone are not used by calls yet.",
    );
    const t = table("Locations", ["The record's name", "Spoken name", "FHIR id", "Address", "Phone", ""]);
    for (const r of d.locations) {
      t.body.append(
        ...row(
          r.record || "A new location",
          [
            this.text("The record's name", r.record, (v) => (r.record = v)),
            this.text("Spoken name", r.spoken, (v) => (r.spoken = v)),
            this.text("FHIR id", r.id, (v) => (r.id = v)),
            this.text("Address", r.address, (v) => (r.address = v)),
            this.text("Phone", r.phone, (v) => (r.phone = v)),
            this.moves(d.locations, r),
          ],
          [
            ["The record's name:", "The Location's name as the record holds it, such as EMC Family Medicine."],
            ["Spoken name:", "What the caller hears. Empty: the record's own words."],
            ["FHIR id:", "Later: Epic's $find location-reference, for \"the north clinic\"."],
          ],
        ),
      );
    }
    out.push(t.wrap, this.add("a location", d.locations, () => ({ record: "", spoken: "", id: "", address: "", phone: "" })));
    return out;
  }

  specialties(): HTMLElement[] {
    const d = this.d;
    const out = heading("Specialties", d.specialties.length, `Epic's specialties: the department's specialty, for "a skin doctor". ${NOT_USED}`);
    out.push(this.listBlock("Specialty list", "specialty_list", "The list every specialty's ID belongs to. Epic gives no list of a hospital's specialties by API."));
    const t = table("Specialties", ["Spoken name", "The caller's words", "ID", ""]);
    for (const r of d.specialties) {
      t.body.append(
        ...row(
          r.spoken || "A new specialty",
          [
            this.text("Spoken name", r.spoken, (v) => (r.spoken = v)),
            this.wordsCell("The caller's words", r.words, (v) => (r.words = v)),
            this.idCell("specialty_list", r.code, (v) => (r.code = v)),
            this.moves(d.specialties, r),
          ],
          [["ID:", "The specialty's ID in the specialty list, from the hospital's own build."]],
        ),
      );
    }
    out.push(t.wrap, this.add("a specialty", d.specialties, () => ({ code: "", spoken: "", words: [] })));
    return out;
  }

  booking(): HTMLElement[] {
    const d = this.d;
    const out = heading("Booking", 6, "How the agent offers slots, finds a caller's visits, and notes a booking.");
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
      ["Search windows (days)", windows, "The search for the visits to come widens through these windows, then has no end. Each window is longer than the one before. One read to Epic per window."],
      [
        "Label after the reason in the booking note",
        this.text("Label after the reason in the booking note", d.booking_note_label, (v) => (d.booking_note_label = v)),
        "Epic's $book files the caller's reason, in their own words, as the visit's note; this label follows it, so staff know who said it. Empty: the words alone. Epic keeps 500 characters; the label is kept whole.",
      ],
    ];
    for (const [name, control, help] of rows) t.body.append(...row(name, [name, control], [["What it does:", help]]));
    out.push(t.wrap);
    return out;
  }
}
