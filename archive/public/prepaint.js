// Appearance pre-paint. index.html loads this file as a plain (blocking) script in <head>,
// so it runs before the stylesheet applies and a dark-mode launch never flashes the light
// look. It is a file, not an inline <script>, because the Content-Security-Policy allows no
// inline script (SECURITY.md §4.3).
//
// theme.ts / skin.ts own the same logic and re-apply it on load: keep the defaults and the
// RETIRED maps here in step with theme.ts and skin.ts.
(function () {
  var RETIRED = {};
  var dark = false;
  try {
    dark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  } catch (e) {}
  [
    ["deets.theme", "data-theme", dark ? "moonlight" : "lilac"],
    ["deets.skin", "data-skin", dark ? "glass" : "press"],
  ].forEach(function (a) {
    var saved = null;
    try {
      saved = localStorage.getItem(a[0]);
    } catch (e) {}
    document.documentElement.setAttribute(a[1], saved ? RETIRED[saved] || saved : a[2]);
  });
})();
