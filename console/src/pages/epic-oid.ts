// What an Epic OID says, in an analyst's words (HOSPITAL-CONFIG.md §2.10; EPIC.md §7.11; the
// owner, 2026-11-15: "translating to master files, id types, and records works best", and "no
// inventing or guessing, we respect Epic's decision and listen to their guidance").
//
// Epic's "ID Types for FHIR APIs" (open.epic tutorial:2659) documents one common pattern,
// 1.2.840.114350.1.13.<CID>.<EnvType>.7.<Value>, and says an app must treat an OID as opaque.
// So this is a label only: the page never builds an OID from it, and nothing is sent from it.
// It reads only the two forms Epic's own examples name:
//   .7.2.<file>          a master file ("1.2.840.114350.1.13.0.1.7.2.657380 – Represents the
//                        Interface Specification (AIP) master file": A=65, I=73, P=80)
//   .7.5.737384.<n>      an ID type ("…7.5.737384.0 – Represents the Epic MRN ID type": an IIT
//                        record, as Epic's "Identity ID Type (IIT) records")
// Any other OID, Epic's or not, gets no label.

export type EpicOid =
  | { kind: "records"; file: string; org: string; env: string }
  | { kind: "idType"; id: string; org: string; env: string };

/** EnvType, in Epic's words (tutorial:2659). */
const ENV: Record<string, string> = { "1": "Epic's own environment", "2": "production", "3": "non-production" };

const PATTERN = /^1\.2\.840\.114350\.1\.13\.(\d+)\.([123])\.7\.(\d+(?:\.\d+)*)$/;

/** A master file's name from its OID node: two digits per letter, each A to Z. */
function letters(node: string): string | null {
  if (node.length < 2 || node.length % 2) return null;
  let out = "";
  for (let i = 0; i < node.length; i += 2) {
    const c = Number(node.slice(i, i + 2));
    if (c < 65 || c > 90) return null;
    out += String.fromCharCode(c);
  }
  return out;
}

/** What `oid` says, or null when it is not one of the forms Epic documents. */
export function readEpicOid(oid: string): EpicOid | null {
  const m = PATTERN.exec(oid.trim().replace(/^urn:oid:/, ""));
  if (!m) return null;
  const [, org, envType, value] = m;
  const env = ENV[envType];
  const parts = value.split(".");
  if (parts[0] === "2" && parts.length === 2) {
    const file = letters(parts[1]);
    return file ? { kind: "records", file, org, env } : null;
  }
  if (parts[0] === "5" && parts.length === 3 && parts[1] === "737384") return { kind: "idType", id: parts[2], org, env };
  return null;
}

/** The prefix a row's ID takes in this list: "R PRC" for a master file's records. */
export function recordPrefix(oid: string): string {
  const o = readEpicOid(oid);
  return o?.kind === "records" ? `R ${o.file}` : "";
}

/** One line for the list's OID, in an analyst's words. */
export function describeEpicOid(oid: string): string {
  if (!oid.trim()) return "Not set.";
  const o = readEpicOid(oid);
  if (!o) return "Not a form Epic's documentation names: kept as entered.";
  const where = `organization ${o.org}, ${o.env}`;
  return o.kind === "records" ? `Records of the master file ${o.file} (${where}).` : `ID type: R IIT ${o.id} (${where}).`;
}
