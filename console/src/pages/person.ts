// What Teams › People's cards and Security › Auth's table share (docs/ops/DEMO.md §3.5): a
// person's picture, and their phone number and its mark. The list itself is the people store
// (agent/stores.ts): a change on one view draws on the other.
//
// Names, emails, phone numbers and pictures here are staff identities. They show only in these
// two views: never in a toast, a hint or a log line.
import { personPhoto, type Person } from "../agent/api";
import { once } from "../agent/store";
import { tokenNumber } from "../tokens";

/** "555-234-5678" for a US number; any other E.164 number as it is. */
export function showPhone(e164: string): string {
  const us = /^\+1(\d{3})(\d{3})(\d{4})$/.exec(e164);
  return us ? `${us[1]}-${us[2]}-${us[3]}` : e164;
}

/** The phone number's mark (the owner, 2026-11-14): a white serif V on forest green when
 * verified, an X on scarlet when not. */
export function phoneMark(verified: boolean): HTMLElement {
  const m = document.createElement("span");
  m.className = `person-mark ${verified ? "is-verified" : "is-unverified"}`;
  m.textContent = verified ? "V" : "X";
  m.title = verified ? "Verified" : "Not verified";
  m.setAttribute("role", "img");
  m.setAttribute("aria-label", verified ? "Verified" : "Not verified");
  return m;
}

// Each picture is fetched once per save: its object URL is kept under the person's id and the
// time it was saved. A new save, or the person gone, frees the old URL.
const photos = new Map<string, { at: string; url: Promise<string | null>; ready?: string }>();

function picture(url: string): HTMLImageElement {
  const img = document.createElement("img");
  img.src = url;
  img.alt = "";
  return img;
}

/** Puts a person's picture in a round slot, or leaves it empty. A picture already loaded goes
 * in at once, so a row drawn again does not blink. */
export function fillPhoto(slot: HTMLElement, p: Person): void {
  if (!p.photo_at) {
    slot.replaceChildren();
    delete slot.dataset.photoAt;
    return;
  }
  let kept = photos.get(p.id);
  if (!kept || kept.at !== p.photo_at) {
    if (kept) void kept.url.then((u) => u && URL.revokeObjectURL(u));
    const entry: { at: string; url: Promise<string | null>; ready?: string } = { at: p.photo_at, url: once("photo", () => personPhoto(p.id)) };
    void entry.url.then((u) => {
      if (u) entry.ready = u;
    });
    kept = entry;
    photos.set(p.id, kept);
  }
  const at = p.photo_at;
  slot.dataset.photoAt = at;
  if (kept.ready) {
    slot.replaceChildren(picture(kept.ready));
    return;
  }
  slot.replaceChildren();
  void kept.url.then((url) => {
    // The slot may have been drawn again for a newer picture while this one loaded.
    if (!url || slot.dataset.photoAt !== at) return;
    slot.replaceChildren(picture(url));
  });
}

/** Frees the pictures of people no longer on the list. */
export function forgetPhotos(list: Person[]): void {
  const ids = new Set(list.map((p) => p.id));
  for (const [id, kept] of photos) {
    if (ids.has(id)) continue;
    void kept.url.then((u) => u && URL.revokeObjectURL(u));
    photos.delete(id);
  }
}

/** A picture file as the store takes it: the middle square, `--photo-px` on a side, WebP. */
export async function squareWebp(file: File): Promise<Blob> {
  const side = tokenNumber("--photo-px", 256);
  const bitmap = await createImageBitmap(file);
  const crop = Math.min(bitmap.width, bitmap.height);
  const canvas = document.createElement("canvas");
  canvas.width = side;
  canvas.height = side;
  const g = canvas.getContext("2d");
  if (!g) throw new Error("this browser cannot draw the picture");
  g.drawImage(bitmap, (bitmap.width - crop) / 2, (bitmap.height - crop) / 2, crop, crop, 0, 0, side, side);
  bitmap.close();
  const blob = await new Promise<Blob | null>((ok) => canvas.toBlob(ok, "image/webp", 0.85));
  // A browser without a WebP encoder gives a PNG instead: refuse it here, with a reason.
  if (!blob || blob.type !== "image/webp") throw new Error("this browser cannot make a WebP picture");
  return blob;
}
