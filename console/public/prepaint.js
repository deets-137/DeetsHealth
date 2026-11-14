// Appearance pre-paint. index.html loads this file as a plain (blocking) script in <head>,
// so it runs before the stylesheet applies and the page never flashes the wrong theme. It is
// a file, not an inline <script>, so the deploy's Content-Security-Policy can refuse every
// inline script (SECURITY.md §4.3).
//
// Keep the key and the defaults in step with src/theme.ts.
(function () {
  var themes = ["dark", "light", "moonlight"];
  var dark = true;
  try {
    dark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  } catch (e) {}
  var saved = null;
  try {
    saved = localStorage.getItem("deets.console.theme");
  } catch (e) {}
  var theme = themes.indexOf(saved) >= 0 ? saved : dark ? "dark" : "light";
  document.documentElement.setAttribute("data-theme", theme);
  document.documentElement.setAttribute("data-skin", "glass");
  // The first-load wave (docs/architecture/LOADING.md §6): the app stays hidden until its data
  // is in, then comes in as one wave (src/reveal.ts). app.css lifts it at --reveal-safety.
  document.documentElement.setAttribute("data-reveal", "hold");
})();
