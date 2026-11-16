// The US states, DC and the territories, by USPS code: what Places › Import from Epic may search
// (HOSPITAL-CONFIG.md §8.2). ehr's hospital.rs › US_STATES holds the same codes and refuses any
// other. Public facts, never protected data.

export const US_STATES: [code: string, name: string][] = [
  ["AL", "Alabama"],
  ["AK", "Alaska"],
  ["AZ", "Arizona"],
  ["AR", "Arkansas"],
  ["CA", "California"],
  ["CO", "Colorado"],
  ["CT", "Connecticut"],
  ["DE", "Delaware"],
  ["DC", "District of Columbia"],
  ["FL", "Florida"],
  ["GA", "Georgia"],
  ["HI", "Hawaii"],
  ["ID", "Idaho"],
  ["IL", "Illinois"],
  ["IN", "Indiana"],
  ["IA", "Iowa"],
  ["KS", "Kansas"],
  ["KY", "Kentucky"],
  ["LA", "Louisiana"],
  ["ME", "Maine"],
  ["MD", "Maryland"],
  ["MA", "Massachusetts"],
  ["MI", "Michigan"],
  ["MN", "Minnesota"],
  ["MS", "Mississippi"],
  ["MO", "Missouri"],
  ["MT", "Montana"],
  ["NE", "Nebraska"],
  ["NV", "Nevada"],
  ["NH", "New Hampshire"],
  ["NJ", "New Jersey"],
  ["NM", "New Mexico"],
  ["NY", "New York"],
  ["NC", "North Carolina"],
  ["ND", "North Dakota"],
  ["OH", "Ohio"],
  ["OK", "Oklahoma"],
  ["OR", "Oregon"],
  ["PA", "Pennsylvania"],
  ["RI", "Rhode Island"],
  ["SC", "South Carolina"],
  ["SD", "South Dakota"],
  ["TN", "Tennessee"],
  ["TX", "Texas"],
  ["UT", "Utah"],
  ["VT", "Vermont"],
  ["VA", "Virginia"],
  ["WA", "Washington"],
  ["WV", "West Virginia"],
  ["WI", "Wisconsin"],
  ["WY", "Wyoming"],
  ["AS", "American Samoa"],
  ["GU", "Guam"],
  ["MP", "Northern Mariana Islands"],
  ["PR", "Puerto Rico"],
  ["VI", "U.S. Virgin Islands"],
];

/** A state's USPS code from its code or its name, in any case; null when it is neither. */
export function stateCode(text: string): string | null {
  const t = text.trim().toLowerCase().replace(/\s+/g, " ");
  const hit = US_STATES.find(([code, name]) => code.toLowerCase() === t || name.toLowerCase() === t);
  return hit ? hit[0] : null;
}

/** What one search entry means, in the form ehr takes ("WI" or "Verona, WI"), or null.
 * Accepts "WI", "wisconsin", "Verona, WI", "verona, wisconsin" and "Verona WI". A city holds
 * letters, spaces, ".", "'" and "-" (at most 60), as ehr's PlaceSearch::parse checks. */
export function placeSearch(text: string): string | null {
  const t = text.trim().replace(/\s+/g, " ");
  if (!t) return null;
  const alone = stateCode(t);
  if (alone) return alone;
  let city: string;
  let state: string | null;
  const comma = t.lastIndexOf(",");
  if (comma >= 0) {
    city = t.slice(0, comma).trim();
    state = stateCode(t.slice(comma + 1));
  } else {
    // "Verona WI": the last word is the state's code.
    const space = t.lastIndexOf(" ");
    if (space < 0) return null;
    city = t.slice(0, space).trim();
    state = /^[a-z]{2}$/i.test(t.slice(space + 1)) ? stateCode(t.slice(space + 1)) : null;
  }
  if (!state || !city || city.length > 60 || !/^[\p{L} .'-]+$/u.test(city)) return null;
  // "verona" reads as "Verona": each word's first letter in capitals.
  const titled = city.replace(/(^|[\s-])(\p{L})/gu, (_, a: string, b: string) => a + b.toUpperCase());
  return `${titled}, ${state}`;
}

/** The "City, ST" of each address on the page ("1 Main St, Verona, WI 53593"), once each. */
export function citiesIn(addresses: string[]): string[] {
  const out = new Set<string>();
  for (const a of addresses) {
    const m = /,\s*([^,]+),\s*([A-Z]{2})(?:\s+\d{5}(?:-\d{4})?)?\s*$/.exec(a);
    const found = m ? placeSearch(`${m[1]}, ${m[2]}`) : null;
    if (found && found.includes(",")) out.add(found);
  }
  return [...out].sort();
}
