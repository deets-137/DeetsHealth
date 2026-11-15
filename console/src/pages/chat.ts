// Chat (CHAT.md): one call in a chat view, in three modes that the page's own nav picks
// (Testing, Demo, PHI), beside the rail of panels.
// - PHI: watch a call. Read only: no controls, no text bar. The rail's second panel is Caller.
// - Test: the user is the caller. Start | End, the text bar; the code button when a text comes.
// - Demo: a recorded call plays back as if live. Play | Stop, the call picker and the speed;
//   the code button when the recording reaches its text.
// In Test and Demo the rail's second panel is the sandbox patients (CHAT.md §8).
// The connection is the hybrid (CHAT.md §5): Test's seat sends over RTVI (agent/transport.ts);
// every mode draws its lines from the agent's events (agent/api.ts › watch, catalog).
import { PipecatClient } from "@pipecat-ai/client-js";
import {
  AgentDown,
  AnnotationRefused,
  annotate,
  callEvents,
  callQuery,
  voiceOffer,
  VERDICTS,
  watch,
  type AnnotationDraft,
  type CallEvent,
  type LiveInfo,
  type CallRow,
  type AnthropicCycle,
  type CallUse,
  type CartesiaCycle,
  type EhrSource,
  type Rating,
  type Sandbox,
  type Verdict,
} from "../agent/api";
import { agentUp, once, type State, type Store } from "../agent/store";
import { catalog, health, resources, sandbox } from "../agent/stores";
import { TextSeat, VoiceSeat } from "../agent/transport";
import { holdReload } from "../reload";
import { revealing } from "../reveal";
import { compactQuery, narrowQuery, reducedMotion, tokenMs, tokenNumber, tokenString } from "../tokens";
import { el, setHtml, type Page } from "./page";
import { placePill } from "./subnav";
import { picker, type Picker } from "../picker";
import { growHandle } from "./panel-grow";

type Mode = "phi" | "test" | "demo";
// The Chat nav's order (the owner, 2026-11-13: "testing, demo and phi").
const MODES: Mode[] = ["test", "demo", "phi"];
const MODE_WORD: Record<Mode, string> = { phi: "PHI", test: "Testing", demo: "Demo" };
const MODE_ABOUT: Record<Mode, string> = {
  phi: "Watch a call. Read only",
  test: "You are the caller",
  demo: "A recorded call plays back as if live",
};
const MODE_KEY = "deets.console.chat.mode";
// Demo's playback speeds (the owner, 2026-11-13: "0.5x, 1x, and 2x"), the speed bar's parts.
const SPEEDS = [0.5, 1, 2];

// The EHR toggle (CHAT.md §13): the EHR the next test call reads.
const EHRS: EhrSource[] = ["local", "epic"];
const EHR_WORD: Record<EhrSource, string> = { local: "Local", epic: "Epic" };
const EHR_ABOUT: Record<EhrSource, string> = {
  local: "Test calls read the local sandbox patients",
  epic: "Test calls read Epic's public sandbox",
};
const EHR_KEY = "deets.console.chat.ehr";

// The seat bar (CHAT.md §17): how the next test call is made. Text each time the page opens and
// after each call ends (the owner, 2026-11-13: "require a manual click of voice every time"),
// so it is never saved.
type Seat = "text" | "voice";
const SEATS: Seat[] = ["text", "voice"];
const SEAT_WORD: Record<Seat, string> = { text: "Text", voice: "Voice" };
// How often the Resources tab reads the credits while a voice call runs.
const RESOURCES_LIVE_MS = 3000;
const savedEhr = (): EhrSource => {
  try {
    const e = localStorage.getItem(EHR_KEY);
    if (e && (EHRS as string[]).includes(e)) return e as EhrSource;
  } catch {
    /* storage blocked */
  }
  return "local";
};

const STAR_ICON = `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 3.5l2.6 5.3 5.9.9-4.3 4.1 1 5.8-5.2-2.7-5.2 2.7 1-5.8-4.3-4.1 5.9-.9z"/></svg>`;
const PLAY_ICON = `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 5.5v13l10.5-6.5z"/></svg>`;
const SEND_ICON =`<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12h13M12 5l7 7-7 7"/></svg>`;

const savedMode = (): Mode => {
  try {
    const m = localStorage.getItem(MODE_KEY);
    if (m && (MODES as string[]).includes(m)) return m as Mode;
  } catch {
    /* storage blocked */
  }
  return "test";
};

export function chatPage(): Page {
  // Set by build(): plays the controls' wave each time the page comes into view.
  let shownHook = () => {};
  // Set by build(): the Chat nav as a dropdown, for the compact bar (picker.ts).
  let modePick: Picker | null = null;
  return {
    id: "chat",
    title: "Chat",
    picker: () => modePick!.el,
    shown: () => requestAnimationFrame(() => shownHook()),
    build() {
      const page = el(`
        <section class="page own-scrollbars">
          <div class="split-page">
            <section class="stage glass chat" aria-labelledby="chat-title" data-mode="test">
              <header class="chat-head">
                <h1 id="chat-title">Chat</h1>
              </header>

              <div class="chat-body">
              <nav class="subnav chat-nav" aria-label="Chat mode">
                <span class="nav-highlight" aria-hidden="true"></span>
                <ul class="nav-list">
                  ${MODES.map(
                    (m) => `<li><button class="nav-item" type="button" data-mode="${m}" title="${MODE_ABOUT[m]}">` +
                      `<span>${MODE_WORD[m]}</span></button></li>`,
                  ).join("")}
                </ul>
              </nav>

              <div class="chat-main">
              <div class="chat-canvas">
                <div class="chat-top">
                  <input class="call-label" data-wave id="call-label" type="text" maxlength="200" autocomplete="off"
                    placeholder="Chat Title (Optional)" aria-label="The next call's title" />
                  <div class="tabs seat" data-wave id="chat-seat" role="radiogroup" aria-label="How you call">
                    <span class="nav-highlight tab-pill" aria-hidden="true"></span>
                    ${SEATS.map(
                      (s) => `<button class="tab" type="button" role="radio" data-seat="${s}" aria-checked="${s === "text"}">${SEAT_WORD[s]}</button>`,
                    ).join("")}
                  </div>
                  <div class="tabs ehr" data-wave id="chat-ehr" role="radiogroup" aria-label="The EHR the test call reads">
                    <span class="nav-highlight tab-pill" aria-hidden="true"></span>
                    ${EHRS.map(
                      (e) => `<button class="tab" type="button" role="radio" data-ehr="${e}" aria-checked="false">${EHR_WORD[e]}</button>`,
                    ).join("")}
                  </div>
                  <div class="tabs speed" data-wave id="demo-speed" role="radiogroup" aria-label="Playback speed">
                    <span class="nav-highlight tab-pill" aria-hidden="true"></span>
                    ${SPEEDS.map(
                      (s) => `<button class="tab" type="button" role="radio" data-speed="${s}" aria-checked="${s === 1}">${s}×</button>`,
                    ).join("")}
                  </div>
                </div>
                <div class="chat-lines" data-for="phi" translate="no" role="log" aria-live="polite" aria-label="The call">
                  <p class="empty">No live call to watch</p>
                </div>
                <div class="chat-lines" data-for="test" translate="no" role="log" aria-live="polite" aria-label="The call">
                  <p class="empty">Press Start to begin a test call</p>
                </div>
                <div class="chat-lines" data-for="demo" translate="no" role="log" aria-live="polite" aria-label="The call">
                  <p class="empty">Pick a recorded call, then press Play</p>
                </div>

                <div class="chat-controls">
                  <div class="help code" id="chat-code" translate="no" hidden>
                    <div class="help-card" id="chat-code-card" role="region" aria-label="The text message">
                      <div class="help-inner"><div class="help-body" id="chat-code-body"></div></div>
                    </div>
                    <div class="split split-code" data-wave role="group" aria-label="The code by text">
                      <button class="split-plain code-show" id="chat-code-show" type="button" aria-expanded="false"
                        aria-controls="chat-code-card"><span id="chat-code-digits"></span></button>
                      <button class="code-send" id="chat-code-send" type="button" aria-label="Send the code as your line"
                        title="Send the code as your line">${SEND_ICON}</button>
                    </div>
                  </div>

                  <div class="split split-call" data-wave role="group" aria-label="The call">
                    <button class="split-go" id="call-start" type="button"><span>Start</span></button>
                    <button class="split-stop" id="call-end" type="button" disabled>
                      <span data-word="end">End call</span><span data-word="confirm" hidden>Confirm?</span>
                    </button>
                  </div>

                  <div class="split split-play" data-wave role="group" aria-label="Playback">
                    <button class="split-go" id="demo-play" type="button" disabled><span>Play</span></button>
                    <button class="split-stop" id="demo-stop" type="button" disabled><span>Stop</span></button>
                  </div>
                </div>
              </div>

              <form class="chat-bar bar-test" id="chat-form" autocomplete="off">
                <input class="chat-input" data-wave id="chat-input" type="text" placeholder="Start a call to type as the caller"
                  aria-label="What the caller says" disabled />
                <button class="send" data-wave id="chat-send" type="submit" aria-label="Send" title="Send (Enter)" disabled>${SEND_ICON}</button>
              </form>
              <!-- A narrow screen only (CONSOLE.md §11.4): the top row's bars move here, under
                   the text box. -->
              <div class="chat-toggles" id="chat-toggles"></div>
              </div>
              </div>
            </section>

            <aside class="rail" aria-label="Chat details">
              <!-- The rail (CHAT.md §15): two panels, each with a tab bar. The top one is the
                   call in plain words; the bottom one is the technical record. -->
              <section class="panel glass rail-panel" id="rail-call" aria-label="The call">
                <div class="tabs" role="tablist" aria-label="The call">
                  <span class="nav-highlight tab-pill" aria-hidden="true"></span>
                  <button class="tab" role="tab" type="button" data-tab="patients"><span class="tab-word-sandbox">Sandbox EPTs</span><span class="tab-word-calls">Recorded calls</span><span class="tab-word-caller">Caller</span></button>
                  <button class="tab" role="tab" type="button" data-tab="steps">Steps</button>
                  <button class="tab" role="tab" type="button" data-tab="review" hidden>Review</button>
                </div>
                <div class="tab-body" role="tabpanel" data-tab="patients">
                  <div class="sandbox" id="sandbox-list" translate="no"><p class="empty">Loading</p></div>
                  <div class="calls" id="demo-calls" translate="no" role="group" aria-label="Recorded calls"><p class="empty">Loading</p></div>
                  <div class="live-calls" id="live-calls" translate="no" role="group" aria-label="Live calls"><p class="empty">No live calls</p></div>
                </div>
                <div class="tab-body" role="tabpanel" data-tab="steps" hidden>
                  ${MODES.map(
                    (m) => `<div class="steps-log" data-for="${m}" translate="no" aria-label="The call's steps">` +
                      `<p class="empty">No call yet</p></div>`,
                  ).join("")}
                </div>
                <div class="tab-body review" role="tabpanel" data-tab="review" id="chat-review" hidden>
                <div class="review-head">
                  <h2 class="review-outcome" id="review-outcome"></h2>
                  <p class="review-call" id="review-call" translate="no"></p>
                  <span class="review-status" id="review-status" role="status"></span>
                  <button class="review-star" id="review-star" type="button" aria-pressed="false"
                    aria-label="Star this call">${STAR_ICON}</button>
                </div>
                <input class="review-input" id="review-label" type="text" maxlength="200" autocomplete="off"
                  placeholder="Label" aria-label="A label for this call" />
                <textarea class="review-input" id="review-note" rows="2" maxlength="2000"
                  placeholder="Note" aria-label="A note on this call"></textarea>
                <div class="review-rows" id="review-rows" translate="no"></div>
                </div>
              </section>

              <section class="panel glass rail-panel" id="rail-tech" aria-label="The technical record">
                <div class="tabs" role="tablist" aria-label="The technical record">
                  <span class="nav-highlight tab-pill" aria-hidden="true"></span>
                  <button class="tab" role="tab" type="button" data-tab="tools">Tools</button>
                  <button class="tab" role="tab" type="button" data-tab="timeline">Timeline</button>
                  <button class="tab" role="tab" type="button" data-tab="resources">Resources</button>
                </div>
                <div class="tab-body" role="tabpanel" data-tab="tools">
                  ${MODES.map(
                    (m) => `<div class="tools-log" data-for="${m}" translate="no" role="log" aria-label="Tool calls">` +
                      `<p class="empty">No tool calls yet</p></div>`,
                  ).join("")}
                </div>
                <div class="tab-body" role="tabpanel" data-tab="timeline" translate="no" hidden>
                  <p class="empty">Empty for now</p>
                </div>
                <div class="tab-body resources" role="tabpanel" data-tab="resources" id="chat-resources" hidden>
                  <p class="empty">Loading</p>
                </div>
              </section>
            </aside>
          </div>
        </section>`);

      const $ = <T extends HTMLElement>(id: string) => page.querySelector<T>(`#${id}`)!;
      const chat = page.querySelector<HTMLElement>(".chat")!;
      const splitPage = page.querySelector<HTMLElement>(".split-page")!;
      const lines = (m: Mode) => page.querySelector<HTMLElement>(`.chat-lines[data-for="${m}"]`)!;
      const toolsBox = (m: Mode) => page.querySelector<HTMLElement>(`.tools-log[data-for="${m}"]`)!;
      const stepsBox = (m: Mode) => page.querySelector<HTMLElement>(`.steps-log[data-for="${m}"]`)!;
      // The rail's panels for a new call in mode `m`: each starts empty.
      const panels = (m: Mode) => ({ tools: new ToolLog(toolsBox(m)), steps: new StepLog(stepsBox(m)) });
      // The rail's two panels and their tab bars (CHAT.md §15).
      const callTabs = new Tabs($("rail-call"), "deets.console.chat.tab.call");
      new Tabs($("rail-tech"), "deets.console.chat.tab.tech");
      // Each panel's grow handle, under the stage only (CHAT.md §15.2).
      growHandle($("rail-call"));
      growHandle($("rail-tech"));

      // ---- the Chat nav (CHAT.md §14): Testing, Demo, PHI, one item per mode. Its pill slides
      // as the big nav's does (subnav.ts › placePill).
      const nav = page.querySelector<HTMLElement>(".chat-nav")!;
      const pill = nav.querySelector<HTMLElement>(".nav-highlight")!;
      const navItems = [...nav.querySelectorAll<HTMLButtonElement>(".nav-item")];
      let mode = savedMode();
      // The same list as a dropdown, for a narrow screen (picker.ts). `goMode` is defined
      // below; a pick runs only after it is.
      modePick = picker("Chat mode", MODES.map((m) => ({ id: m, title: MODE_WORD[m] })), "right", (id) => goMode(id as Mode));
      const markNav = (animate: boolean) => {
        modePick!.set(mode);
        navItems.forEach((b) => {
          if (b.dataset.mode === mode) b.setAttribute("aria-current", "page");
          else b.removeAttribute("aria-current");
        });
        placePill(pill, navItems.find((b) => b.dataset.mode === mode), animate);
      };
      // Also places the pill the first time the page is shown, when the nav first has a size.
      new ResizeObserver(() => markNav(false)).observe(nav);

      // ---- a narrow screen (CONSOLE.md §11.4, the owner, 2026-11-14): the seat, EHR and speed
      // bars move from the canvas's top row to a row under the text box, and back when the
      // window grows. Each bar keeps its handlers; its pill follows on its next resize.
      const topRow = page.querySelector<HTMLElement>(".chat-top")!;
      const underRow = $("chat-toggles");
      const bars = ["chat-seat", "chat-ehr", "demo-speed"].map($);
      const compact = compactQuery();
      const placeBars = () => (compact.matches ? underRow : topRow).append(...bars);
      compact.addEventListener("change", placeBars);
      placeBars();

      // ---- the call panel first (CHAT.md §15.1; the owner's fork A, 2026-11-14): while the rail
      // sits under the stage, Demo and PHI show the rail's top panel (Recorded calls, Caller)
      // above the stage until a call is picked in this visit to the mode. A pick slides the
      // panel back under the stage, and the stage up. Entering Demo or PHI again starts over.
      const railCall = $("rail-call");
      const rail = page.querySelector<HTMLElement>(".rail")!;
      const narrow = narrowQuery();
      let picked = false; // a call was picked since the mode was entered
      let swap: Animation | null = null; // the half of the move that is running
      const placeCall = (animate: boolean) => {
        const above = narrow.matches && mode !== "test" && !picked;
        swap?.cancel(); // a newer move wins: the half that was running stops where it is
        if (above === (railCall.parentElement === splitPage)) return;
        const move = () => {
          const focused = railCall.contains(document.activeElement) ? (document.activeElement as HTMLElement) : null;
          if (above) splitPage.prepend(railCall);
          else rail.prepend(railCall);
          splitPage.toggleAttribute("data-call-above", above);
          if (!above) page.scrollTop = 0; // the stage comes into view
          focused?.focus({ preventScroll: true }); // a move can drop the focus
        };
        if (!animate || reducedMotion() || !chat.offsetParent) return move();
        // The panels never cross (the owner, 2026-11-14): the panel closes where it is, so the
        // panels after it glide up into its room; then it opens in its new place, so the
        // panels after that glide down. Its height, padding and the gap after it all go to 0
        // and back, so nothing jumps. Nothing is held after the motion.
        const cs = getComputedStyle(railCall);
        const shut: Keyframe = {
          height: "0px",
          minHeight: "0px",
          paddingTop: "0px",
          paddingBottom: "0px",
          marginBottom: `-${tokenNumber("--gap", 12)}px`,
          opacity: 0,
        };
        const full = (h: number): Keyframe => ({
          height: `${h}px`,
          minHeight: `${h}px`,
          paddingTop: cs.paddingTop,
          paddingBottom: cs.paddingBottom,
          marginBottom: "0px",
          opacity: 1,
        });
        const timing = { duration: tokenMs("--dur-swap") / 2, easing: tokenString("--ease", "ease") };
        railCall.classList.add("swapping");
        // Only the newest half clears the state: a newer move may have started already.
        const done = (a: Animation) => () => {
          if (swap !== a) return;
          swap = null;
          railCall.classList.remove("swapping");
        };
        const close = railCall.animate([full(railCall.getBoundingClientRect().height), shut], { ...timing, fill: "forwards" });
        swap = close;
        close.finished.then(() => {
          if (swap !== close) return;
          move();
          close.cancel(); // the panel lays out at its own size in its new place...
          const open = railCall.animate([shut, full(railCall.getBoundingClientRect().height)], timing);
          swap = open; // ...and opens from shut to that size
          open.finished.then(done(open), done(open));
        }, done(close)); // cancelled by a newer move, which has put the panel in its place
      };
      const pickedCall = () => {
        if (picked) return;
        picked = true;
        placeCall(true);
      };
      narrow.addEventListener("change", () => placeCall(false));
      const showMode = () => {
        chat.dataset.mode = mode;
        splitPage.dataset.mode = mode;
        callTabs.place(false); // its first tab's word: Sandbox EPTs, or Caller in PHI
        MODES.forEach((m) => {
          lines(m).hidden = m !== mode;
          toolsBox(m).hidden = m !== mode;
          stepsBox(m).hidden = m !== mode;
        });
        closeCode();
        showSandbox(); // the rail's list follows the mode: Test's toggle, Demo's call
        showReview(false);
        placeCall(true); // Demo and PHI: the call panel above the stage till a pick
      };
      // ---- the wave (chat.css › the wave): the visible controls come in one after another,
      // top row first, left to right, as one wave.
      let waveTimer = 0;
      const waveIn = () => {
        // On a first load the screen's own wave brings these in (reveal.ts): never two waves.
        if (reducedMotion() || revealing()) return;
        const parts = [...chat.querySelectorAll<HTMLElement>("[data-wave]")]
          .filter((p) => p.getClientRects().length > 0)
          .map((p) => ({ p, r: p.getBoundingClientRect() }))
          .sort((a, b) => Math.round(a.r.top) - Math.round(b.r.top) || a.r.left - b.r.left);
        parts.forEach(({ p }, i) => p.style.setProperty("--wave-i", String(i)));
        chat.classList.remove("wave-in");
        void chat.offsetWidth; // restart the animation
        chat.classList.add("wave-in");
        window.clearTimeout(waveTimer);
        const total = tokenMs("--dur-wave-in") + parts.length * tokenMs("--wave-step");
        waveTimer = window.setTimeout(() => chat.classList.remove("wave-in"), total);
      };

      let swapping = 0;
      const goMode = (next: Mode) => {
        if (next === mode) return;
        mode = next;
        picked = false; // a new visit to the mode: the call panel comes first again
        try {
          localStorage.setItem(MODE_KEY, mode);
        } catch {
          /* the mode holds for this visit only */
        }
        markNav(true); // the pill starts its slide at once
        void loadResources(); // the Resources tab shows this mode's call (CHAT.md §17)
        if (reducedMotion()) return showMode();
        // The old controls fade out together; then the mode swaps and the new controls come in
        // as a wave. The latest press wins when presses come quickly.
        const mine = ++swapping;
        chat.classList.remove("wave-in");
        chat.classList.add("wave-out");
        window.setTimeout(() => {
          if (mine !== swapping) return;
          // The swap always finishes: if showMode throws, the controls still come back (the
          // owner, 2026-11-13: the buttons stayed hidden till a reload).
          try {
            showMode();
          } finally {
            chat.classList.remove("wave-out");
            waveIn();
          }
        }, tokenMs("--dur-wave-out"));
      };
      nav.addEventListener("click", (e) => {
        const b = (e.target as HTMLElement).closest<HTMLButtonElement>(".nav-item");
        if (b) goMode(b.dataset.mode as Mode);
      });
      // Up and Down move between modes, as in the big nav.
      nav.addEventListener("keydown", (e) => {
        if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
        const at = navItems.indexOf(document.activeElement as HTMLButtonElement);
        if (at < 0) return;
        e.preventDefault();
        const next = navItems[(at + (e.key === "ArrowDown" ? 1 : -1) + navItems.length) % navItems.length];
        next.focus();
        goMode(next.dataset.mode as Mode);
      });
      shownHook = waveIn;

      // ---- the Review panel (CHAT.md §12): the call each mode last finished, Test's from its
      // own call, Demo's from the playback that ran to its end. PHI rates nothing.
      const review = new Review(
        $("chat-review"),
        $("review-status"),
        $<HTMLButtonElement>("review-star"),
        $("review-outcome"),
        $("review-call"),
        $<HTMLInputElement>("review-label"),
        $<HTMLTextAreaElement>("review-note"),
        $("review-rows"),
        reviewShown(callTabs),
      );
      const reviewFor: Record<Mode, string | null> = { phi: null, test: null, demo: null };
      function showReview(slide: boolean) {
        const id = reviewFor[mode];
        if (!id) return review.close();
        if (review.current !== id) void review.open(id, "", slide);
      }
      const finished = (m: Mode, id: string) => {
        reviewFor[m] = id;
        if (m === mode) showReview(true);
      };
      const unreview = (m: Mode) => {
        reviewFor[m] = null;
        if (m === mode) showReview(false);
      };

      // ---- the sandbox patients (the rail, Test and Demo): from the dev server. Local: the
      // same file ehr's fakes read (ehr/fixtures/sandbox.json). Epic: Epic's public test
      // patients, read live through ehr (CHAT.md §13). Test shows the EHR the toggle picks; Demo
      // the EHR of the call it plays.
      const sandboxList = $("sandbox-list");
      const boxes: Partial<Record<EhrSource, Sandbox>> = {};
      const failed: Partial<Record<EhrSource, string>> = {};
      let ehr = savedEhr();
      let demoEhr: EhrSource = "local";
      const railEhr = (): EhrSource => (mode === "demo" ? demoEhr : ehr);
      let box: Sandbox | undefined; // the list the rail shows now
      const loading = new Set<EhrSource>(); // the reads in flight, so a list is read once
      const showSandbox = () => {
        const source = railEhr();
        box = boxes[source];
        if (box) sandboxList.replaceChildren(...sandboxRows(box));
        else if (failed[source]) sandboxList.replaceChildren(note(failed[source]!));
        else {
          sandboxList.replaceChildren(note(source === "epic" ? "Reading Epic's sandbox" : "Loading"));
          // Nothing else reads a list the rail needs now (the owner, 2026-11-13: "still
          // loading?" after Testing on Epic, then Demo): read it here.
          if (!loading.has(source)) void loadSandbox(source);
        }
        renderCode(false);
      };
      // Each EHR's list is a store (LOADING.md §3): read here, and when it fails because the
      // agent is down, read again by the store when the agent is back; the subscription then
      // fills the rail. Epic's list leaves this machine, so it is never read on focus.
      const following = new Set<EhrSource>();
      const loadSandbox = async (source: EhrSource = railEhr()) => {
        if (loading.has(source)) return;
        loading.add(source);
        delete failed[source];
        if (source === railEhr()) showSandbox();
        const st = sandbox(source);
        await st.refresh("open");
        const got = st.get();
        if (got.status === "ready" && got.value) boxes[source] = got.value;
        else
          failed[source] =
            got.status === "refused"
              ? source === "epic"
                ? "Epic's sandbox did not answer"
                : "The agent's dev server refused"
              : "The agent's dev server is not running";
        loading.delete(source);
        if (!following.has(source)) {
          following.add(source);
          st.subscribe(sandboxList, (s) => {
            if (loading.has(source) || s.status !== "ready" || !s.value) return;
            boxes[source] = s.value;
            delete failed[source];
            if (source === railEhr()) showSandbox();
          });
        }
        if (source === railEhr()) showSandbox();
      };

      // ---- the EHR bar (CHAT.md §13): Local | Epic, a tab bar like the seat and speed bars. At
      // the canvas's top right, Testing only; locked while a test call runs. The choice is kept.
      const ehrBar = $("chat-ehr");
      const ehrPill = ehrBar.querySelector<HTMLElement>(".tab-pill")!;
      const ehrParts = [...ehrBar.querySelectorAll<HTMLButtonElement>("[data-ehr]")];
      let epicServed = true; // until the dev server says otherwise
      let ehrAnimate = false; // the pill slides only after a press
      const showEhr = () => {
        ehrParts.forEach((b) => {
          const e = b.dataset.ehr as EhrSource;
          b.setAttribute("aria-checked", String(e === ehr));
          b.disabled = live || seeking || (e === "epic" && !epicServed);
          b.title =
            e === "epic" && !epicServed
              ? "Epic mode needs epic-client.json and the non-production key"
              : live || seeking
                ? "The EHR is fixed for the call in progress"
                : EHR_ABOUT[e];
        });
        placePill(ehrPill, ehrParts.find((b) => b.dataset.ehr === ehr), ehrAnimate);
        ehrAnimate = false;
      };
      ehrBar.addEventListener("click", (e) => {
        const b = (e.target as HTMLElement).closest<HTMLButtonElement>("[data-ehr]");
        if (!b || b.disabled || live || seeking || b.dataset.ehr === ehr) return;
        ehr = b.dataset.ehr as EhrSource;
        try {
          localStorage.setItem(EHR_KEY, ehr);
        } catch {
          /* the choice holds for this visit only */
        }
        ehrAnimate = true;
        showEhr();
        if (boxes[ehr] || failed[ehr] === undefined) showSandbox();
        if (!boxes[ehr]) void loadSandbox(ehr);
      });
      // Places the pill once the bar has a size (it has none outside Testing).
      new ResizeObserver(() => showEhr()).observe(ehrBar);
      // What the dev server offers (Epic, voice): the health store, read once the page builds
      // and again by the store when the agent comes back.
      health.subscribe(ehrBar, (s) => {
        const h = s.value;
        if (s.status !== "ready" || !h) return;
        epicServed = h.epic;
        if (!h.epic && ehr === "epic") {
          ehr = "local";
          showSandbox();
        }
        showEhr();
        voiceServed = h.voice;
        showSeat(false);
      });

      // ---- the seat bar (CHAT.md §17): Text | Voice, at the canvas's top right beside the EHR
      // toggle, Testing only. Text each time the page opens and after each call; locked while a
      // call runs; Voice is off without the Cartesia key.
      const seatBar = $("chat-seat");
      const seatPill = seatBar.querySelector<HTMLElement>(".tab-pill")!;
      const seatParts = [...seatBar.querySelectorAll<HTMLButtonElement>("[data-seat]")];
      let seat: Seat = "text";
      let voiceServed = false; // until the dev server says it has the key
      function showSeat(animate: boolean) {
        seatParts.forEach((b) => {
          const s = b.dataset.seat as Seat;
          b.setAttribute("aria-checked", String(s === seat));
          b.disabled = live || seeking || (s === "voice" && !voiceServed);
          b.title =
            s === "voice" && !voiceServed
              ? "A voice call needs a Cartesia key in .env"
              : live || seeking
                ? "Fixed for the call in progress"
                : s === "voice"
                  ? "Speak through the microphone; you can still type"
                  : "Type as the caller";
        });
        placePill(seatPill, seatParts.find((b) => b.dataset.seat === seat), animate);
      }
      seatBar.addEventListener("click", (e) => {
        const b = (e.target as HTMLElement).closest<HTMLButtonElement>("[data-seat]");
        if (!b || b.disabled || live || seeking) return;
        seat = b.dataset.seat as Seat;
        showSeat(true);
      });
      // Places the pill once the bar has a size (it has none outside Testing).
      new ResizeObserver(() => showSeat(false)).observe(seatBar);

      // ---- the code by text (CHAT.md §8): when the fake Twilio takes a text for a test call,
      // or a recorded call reaches its text, the code button comes in at the bottom left. Its
      // left half shows the six digits and opens the text; its right half sends the code as
      // the caller's line (a live test call only). Each mode keeps its own texts.
      type Texted = { to: string; body: string; code: string };
      const texts: Record<Mode, Texted[]> = { phi: [], test: [], demo: [] };
      const code = $("chat-code");
      const codeShow = $<HTMLButtonElement>("chat-code-show");
      const codeDigits = $("chat-code-digits");
      const codeSend = $<HTMLButtonElement>("chat-code-send");
      const codeBody = $("chat-code-body");
      const closeCode = () => {
        code.classList.remove("open");
        codeShow.setAttribute("aria-expanded", "false");
      };
      // PHI never shows a code: staff never see a second factor, in dev as in production.
      const shownTexts = () => (mode === "phi" ? [] : texts[mode]);
      function renderCode(arrived: boolean) {
        const list = shownTexts();
        const latest = list[list.length - 1];
        code.hidden = !latest;
        if (!latest) return closeCode();
        codeDigits.textContent = latest.code;
        codeShow.setAttribute("aria-label", `Code ${latest.code.split("").join(" ")}. Show the text message`);
        codeSend.disabled = mode !== "test" || !live;
        codeBody.replaceChildren(...textCards(list, box));
        if (arrived && !reducedMotion()) {
          code.classList.remove("arrive");
          void code.offsetWidth; // restart the animation
          code.classList.add("arrive");
        }
      }
      const addText = (m: Mode, e: CallEvent) => {
        if (typeof e.to !== "string" || typeof e.body !== "string") return;
        const digits = /\b(\d{6})\b/.exec(e.body)?.[1];
        if (!digits) return;
        texts[m].push({ to: e.to, body: e.body, code: digits });
        if (m === mode) renderCode(true);
      };
      const clearTexts = (m: Mode) => {
        texts[m] = [];
        if (m === mode) renderCode(false);
      };
      code.addEventListener("animationend", () => code.classList.remove("arrive"));
      codeShow.addEventListener("click", () => {
        const open = !code.classList.contains("open");
        code.classList.toggle("open", open);
        codeShow.setAttribute("aria-expanded", String(open));
      });
      codeSend.addEventListener("click", () => {
        const latest = texts.test[texts.test.length - 1];
        if (mode !== "test" || !live || !client || !latest) return;
        // In a voice call the agent speaks its answer to a typed line too.
        void client.sendText(latest.code, { run_immediately: true, audio_response: callSeat === "voice" }).catch(() => {});
        closeCode();
        input.focus();
      });

      // ---- a test call: Start | End, with End asking "Confirm?" first
      const start = $<HTMLButtonElement>("call-start");
      const end = $<HTMLButtonElement>("call-end");
      const endWord = end.querySelector<HTMLElement>('[data-word="end"]')!;
      const confirmWord = end.querySelector<HTMLElement>('[data-word="confirm"]')!;
      const input = $<HTMLInputElement>("chat-input");
      const send = $<HTMLButtonElement>("chat-send");
      // The label typed beside Start (docs/ops/DEMO.md §3.6): sent with the call, shown in the
      // Caller rail's list at once, and the call's label in the catalog when it ends.
      const labelBox = $<HTMLInputElement>("call-label");
      const testLines = lines("test");
      let live = false;
      let confirmTimer = 0;

      const setConfirm = (on: boolean) => {
        window.clearTimeout(confirmTimer);
        endWord.hidden = on;
        confirmWord.hidden = !on;
        end.setAttribute("aria-label", on ? "Confirm: end the call" : "End call");
        if (on) confirmTimer = window.setTimeout(() => setConfirm(false), tokenMs("--confirm-hold"));
      };
      const setLive = (on: boolean) => {
        live = on;
        // The demo's live reload waits for this tab's call to end (reload.ts).
        holdReload("chat:call", on);
        setConfirm(false);
        start.disabled = on;
        labelBox.disabled = on;
        // A finished call's label is done with: the next call starts blank.
        if (!on) labelBox.value = "";
        end.disabled = !on;
        input.disabled = !on;
        send.disabled = !on;
        input.placeholder = !on
          ? "Start a call to type as the caller"
          : callSeat === "voice"
            ? "Speak, or type as the caller and press Enter"
            : "Type as the caller, then press Enter";
        // Back to Text after each call: a voice call takes a fresh press of Voice.
        if (!on) seat = "text";
        renderCode(false);
        showEhr();
        showSeat(true);
        watchResources();
      };
      const note = (text: string) => {
        const p = document.createElement("p");
        p.className = "empty";
        p.textContent = text;
        return p;
      };

      // The lines of a test call come back from the watch stream, the same path PHI and Demo
      // draw from; the seat only sends (CHAT.md §5).
      let client: PipecatClient | undefined;
      let callSeat: Seat = "text"; // the seat of the call in progress
      let myCall: string | null = null;
      // This page's mark for the call it starts: the dev server sends it back on "started", so
      // the page knows its own call among several (docs/ops/DEMO.md §3.6).
      let myTag = "";
      let seeking = false; // between Start and the dev server's "started"
      let testView: CallView | undefined;
      const hangUp = async () => {
        const c = client;
        client = undefined;
        await c?.disconnect().catch(() => {});
      };

      start.addEventListener("click", async () => {
        start.disabled = true;
        testLines.replaceChildren(note("Calling the agent…"));
        clearTexts("test");
        unreview("test");
        testView = new CallView(testLines, { onText: (e) => addText("test", e), ...panels("test") });
        myCall = null;
        myTag = crypto.randomUUID();
        seeking = true;
        labelBox.disabled = true;
        callSeat = seat;
        showEhr();
        showSeat(false);
        const query = callQuery(ehr, labelBox.value.trim(), myTag);
        client = new PipecatClient({
          // The EHR the toggle picks and the seat are fixed for the whole call (CHAT.md §13, §17).
          transport:
            callSeat === "voice"
              ? new VoiceSeat((offer) => voiceOffer(query, offer))
              : new TextSeat(`/agent/call?${query}`),
          enableMic: callSeat === "voice",
          enableCam: false,
          callbacks: {
            onDisconnected: () => {
              if (live) {
                setLive(false);
                testLines.append(note("Call ended"));
              }
            },
          },
        });
        try {
          await client.connect();
          setLive(true);
          // The greeting may already be on the canvas (it comes over the watch stream).
          if (!testLines.querySelector(".line")) testLines.replaceChildren(note("Connected"));
          input.focus();
        } catch (e) {
          client = undefined;
          seeking = false;
          showEhr();
          showSeat(false);
          start.disabled = false;
          labelBox.disabled = false; // the label stays, for another try
          testLines.replaceChildren(
            note(e instanceof Error && e.message ? `No call: ${e.message}` : "No call: the agent did not answer"),
          );
        }
      });
      end.addEventListener("click", () => {
        if (!live) return;
        if (confirmWord.hidden) return setConfirm(true);
        setLive(false);
        void hangUp();
        start.focus();
      });
      end.addEventListener("keydown", (e) => {
        if (e.key === "Escape" && !confirmWord.hidden) setConfirm(false);
      });

      $<HTMLFormElement>("chat-form").addEventListener("submit", (e) => {
        e.preventDefault();
        const text = input.value.trim();
        if (!live || !text || !client) return;
        void client.sendText(text, { run_immediately: true, audio_response: callSeat === "voice" }).catch(() => {});
        input.value = "";
      });

      // ---- the watch stream: every live call's events (docs/ops/DEMO.md §3.6: several at once).
      // Test draws its own call; PHI draws the call picked in the Caller tab's list, or the
      // newest until one is picked.
      const phiLines = lines("phi");
      let phiView: CallView | undefined;
      let phiCall: string | null = null;
      let phiPicked = false; // the person chose phiCall in the list; a new call does not move it
      // Each live call: what the dev server says of it, and its events so far, so a pick can
      // draw the call from its start.
      const lives = new Map<string, { info: LiveInfo; events: CallEvent[] }>();
      const liveList = $("live-calls");
      const phiIdle = () => phiLines.replaceChildren(note("No live call to watch"));
      const watchCall = (id: string | null) => {
        phiCall = id;
        if (!id) return phiIdle();
        // PHI shows every field in dev (synthetic), as CHAT.md §2.4 decided; roles gate it later.
        phiView = new CallView(phiLines, panels("phi"));
        phiLines.replaceChildren();
        for (const e of lives.get(id)?.events ?? []) phiView.add(e);
        drawLives();
      };
      function drawLives() {
        liveList.replaceChildren(
          ...(lives.size
            ? [...lives.values()].reverse().map((c) => liveRow(c.info, c.events, c.info.call === phiCall))
            : [note("No live calls")]),
        );
      }
      liveList.addEventListener("click", (e) => {
        const r = (e.target as HTMLElement).closest<HTMLElement>(".call-row");
        if (!r?.dataset.id || !lives.has(r.dataset.id)) return;
        phiPicked = true;
        watchCall(r.dataset.id);
        void loadResources();
        pickedCall();
      });
      watch(
        (m) => {
          if (m.type === "hello") {
            lives.clear();
            for (const info of m.calls) lives.set(info.call, { info, events: [] });
            if (!phiCall || !lives.has(phiCall)) {
              phiPicked = false;
              watchCall(m.live);
            } else drawLives();
            watchResources();
            return;
          }
          if (m.type === "started") {
            lives.set(m.call, { info: m.info, events: [] });
            if (seeking && m.info.tag === myTag) {
              myCall = m.call;
              seeking = false;
              showEhr();
            }
            if (!phiPicked) watchCall(m.call);
            else drawLives();
            watchResources();
            return;
          }
          if (m.type === "event") {
            const c = lives.get(m.call);
            c?.events.push(m.event);
            if (m.call === phiCall) phiView?.add(m.event);
            if (m.call === myCall) testView?.add(m.event);
            // The list's step and turn counts change with these only.
            if (c && (m.event.kind === "step" || m.event.kind === "caller")) drawLives();
            return;
          }
          if (m.type === "ended") {
            lives.delete(m.call);
            if (m.call === phiCall) {
              phiCall = null;
              phiPicked = false;
              phiLines.append(note("The call ended"));
            }
            drawLives();
            if (m.call === myCall && live) {
              setLive(false);
              void hangUp();
            }
            // The dev server adds the call to the catalog before it says "ended".
            if (m.call === myCall) finished("test", m.call);
            void loadCatalog();
            watchResources();
          }
        },
        (up) => {
          // The socket reached the agent: every store that found it down reads again now.
          if (up) agentUp();
          if (!up) {
            lives.clear();
            drawLives();
          }
          if (!up && !phiCall) phiLines.replaceChildren(note("The agent's dev server is not running"));
          else if (up && !phiCall) phiIdle();
          if (up && !box) void loadSandbox();
        },
      );

      // ---- the Resources tab (CHAT.md §17): Claude's dollars and Cartesia's credits left this
      // cycle, and the use of the call this page shows: Testing's own call (kept after it ends,
      // till the next Start), the call PHI watches, the call Demo played. Read when the page
      // opens, when the shown call changes, after each call, and every few seconds while any
      // call runs (local reads only: no request to a vendor).
      // One store per shown call (LOADING.md §3). The tab follows the store of the call shown
      // now; a change of call moves the subscription, so a late answer for the call shown
      // before never draws. While a call runs the store polls, 3 s after each answer, each
      // tick in the ledger as a `poll` of `resources` (the call id never goes in).
      const resourcesBox = $("chat-resources");
      let demoPlayed: string | null = null;
      const shownCall = () => (mode === "test" ? myCall : mode === "phi" ? phiCall : demoPlayed);
      type Use = Awaited<ReturnType<typeof import("../agent/api").resources>>;
      let useShown: Store<Use> | null = null;
      let stopFollow = () => {};
      let stopPoll = () => {};
      const drawUse = (s: State<Use>) => {
        const r = s.value;
        if (r) resourcesBox.replaceChildren(...resourceRows(r.anthropic, r.cartesia, r.call, r.voice));
        else if (s.status === "down") resourcesBox.replaceChildren(note("The agent's dev server is not running"));
        else if (s.status === "refused") resourcesBox.replaceChildren(note("The agent's dev server refused"));
      };
      const loadResources = () => {
        const st = resources(shownCall());
        if (st !== useShown) {
          stopFollow();
          useShown = st;
          // A first subscription reads; a store read before draws what it has, then reads.
          stopFollow = st.subscribe(resourcesBox, (s) => st === useShown && drawUse(s));
          if (st.get().status === "loading") return;
        }
        void st.refresh("user");
      };
      function watchResources() {
        stopPoll();
        loadResources();
        const st = useShown;
        if (st && (live || lives.size > 0))
          stopPoll = st.poll(RESOURCES_LIVE_MS, () => st === useShown && (live || lives.size > 0));
      }
      loadResources();

      // ---- Demo: a recorded call plays back as if live, from the catalog
      const play = $<HTMLButtonElement>("demo-play");
      const stop = $<HTMLButtonElement>("demo-stop");
      const demoLines = lines("demo");

      // The speed bar (CHAT.md §16): 0.5×, 1×, 2× in the canvas's top row, a pill on the
      // chosen part. A change takes effect at the next Play.
      const speedBar = $("demo-speed");
      const speedPill = speedBar.querySelector<HTMLElement>(".tab-pill")!;
      const speedParts = [...speedBar.querySelectorAll<HTMLButtonElement>("[data-speed]")];
      let speed = 1;
      const markSpeed = (animate: boolean) => {
        speedParts.forEach((b) => b.setAttribute("aria-checked", String(Number(b.dataset.speed) === speed)));
        placePill(speedPill, speedParts.find((b) => Number(b.dataset.speed) === speed), animate);
      };
      speedBar.addEventListener("click", (e) => {
        const b = (e.target as HTMLElement).closest<HTMLButtonElement>("[data-speed]");
        if (!b) return;
        speed = Number(b.dataset.speed) || 1;
        markSpeed(true);
      });
      // Places the pill once the bar has a size (it has none outside Demo).
      new ResizeObserver(() => markSpeed(false)).observe(speedBar);

      // The recorded calls: the rail's first tab in Demo (CHAT.md §16), starred first, then
      // newest. A press chooses a call; Play plays the chosen one.
      const callsList = $("demo-calls");
      let chosen: string | null = null;
      let timers: number[] = [];
      const canPlay = () => chosen !== null && timers.length === 0;
      const markChosen = () =>
        callsList.querySelectorAll<HTMLElement>(".call-row").forEach((r) => {
          const on = r.dataset.id === chosen;
          r.classList.toggle("is-chosen", on);
          r.querySelector(".call-pick")!.setAttribute("aria-pressed", String(on));
        });
      callsList.addEventListener("click", (e) => {
        const t = e.target as HTMLElement;
        const r = t.closest<HTMLElement>(".call-row");
        if (!r) return;
        chosen = r.dataset.id!;
        markChosen();
        play.disabled = !canPlay();
        pickedCall();
        // The row's own play button plays it at once (the owner, 2026-11-13).
        if (t.closest(".call-play")) void startDemo();
      });
      const stopDemo = () => {
        timers.forEach((t) => window.clearTimeout(t));
        timers = [];
        stop.disabled = true;
        play.disabled = !canPlay();
      };
      // The catalog store (LOADING.md §3): the list draws on each answer, from this page's reads,
      // a review's annotation (which reads it again), and the store's own read when the agent
      // is back. A list already drawn stays while a later read fails.
      catalog.subscribe(callsList, (s) => {
        if (!s.value) {
          if (s.status === "down" || s.status === "refused") {
            callsList.replaceChildren(note(s.status === "down" ? "The agent's dev server is not running" : "The agent's dev server refused"));
            chosen = null;
            play.disabled = true;
          }
          return;
        }
        const calls: CallRow[] = s.value.calls;
        if (!calls.some((c) => c.id === chosen)) chosen = calls[0]?.id ?? null;
        callsList.replaceChildren(...(calls.length ? calls.map(callRow) : [note("No recorded calls yet")]));
        markChosen();
        play.disabled = !canPlay();
      });
      function loadCatalog() {
        return catalog.refresh("user");
      }
      play.addEventListener("click", () => {
        pickedCall(); // Play on the call chosen for you counts as a pick
        void startDemo();
      });
      async function startDemo() {
        if (!chosen) return;
        const id = chosen; // the call to play, even if another is chosen while it loads
        demoPlayed = id;
        void loadResources();
        stopDemo();
        play.disabled = true;
        stop.disabled = false;
        let events: CallEvent[];
        try {
          // A one-off read, never kept in a store: a call's events are its transcript.
          events = await once("events", () => callEvents(id));
        } catch {
          demoLines.replaceChildren(note("That call could not be read"));
          return stopDemo();
        }
        demoLines.replaceChildren();
        clearTexts("demo");
        unreview("demo");
        // The rail shows the patients of the EHR this call read (schema 6; older calls: local).
        const start0 = events.find((e) => e.kind === "call_start");
        demoEhr = start0?.ehr === "epic" ? "epic" : "local";
        if (mode === "demo") showSandbox();
        if (!boxes[demoEhr]) void loadSandbox(demoEhr);
        const view = new CallView(demoLines, { onText: (e) => addText("demo", e), ...panels("demo") });
        const last = events.reduce((m, e) => Math.max(m, e.t_ms), 0);
        for (const e of events) timers.push(window.setTimeout(() => view.add(e), e.t_ms / speed));
        timers.push(
          window.setTimeout(() => {
            stopDemo();
            finished("demo", id); // played to its end: rate it
          }, last / speed + 50),
        );
      }
      stop.addEventListener("click", () => {
        stopDemo();
        demoLines.append(note("Stopped"));
      });
      // (The catalog reads on its own: its store's subscription above.)
      void loadSandbox();
      showEhr();

      markNav(false);
      showMode();
      return page;
    },
  };
}

/** A call drawn from its events, live or played back: the same code for Test, PHI and Demo. */
class CallView {
  private llm: CallEvent[] = [];
  /** `onText`: a text message the call sent (the code by text); it shows outside the lines.
   * `tools`: the rail's Tools panel for this call. */
  constructor(
    private readonly box: HTMLElement,
    private readonly hooks: { onText?: (e: CallEvent) => void; tools?: ToolLog; steps?: StepLog } = {},
  ) {}

  add(e: CallEvent): void {
    // The Step panel reads every kind it needs itself (steps, limits, marks, the end).
    this.hooks.steps?.add(e);
    if (e.kind === "llm") {
      this.llm.push(e);
      return;
    }
    if (e.kind === "text") {
      this.hooks.onText?.(e);
      return;
    }
    if (e.kind === "tool_call") return this.hooks.tools?.call(e);
    if (e.kind === "tool_result") return this.hooks.tools?.result(e);
    if (e.kind === "caller") this.hooks.tools?.caller(e);
    if (e.kind === "agent") this.hooks.tools?.agent(e);
    if (e.kind === "caller" && typeof e.text === "string") {
      this.llm = [];
      this.show(line("caller", e.text));
    } else if (e.kind === "agent" && typeof e.text === "string") {
      this.show(line("agent", e.text, agentFacts(this.llm)));
      this.llm = [];
    } else if (e.kind === "call_end") {
      this.show(endNote(typeof e.reason === "string" ? e.reason : ""));
    }
  }

  private show(node: HTMLElement): void {
    this.box.querySelector(".empty")?.remove();
    this.box.append(node);
    this.box.scrollTop = this.box.scrollHeight;
  }
}

/** The words for each rubric dimension (a draft, CHAT.md §12). A dimension not listed shows
 * its key in words. The dimensions themselves come from the catalog (catalog.py). */
const DIMENSION_WORDS: Record<string, string> = {
  right_outcome: "Right outcome",
  verification: "Verification",
  no_invented_values: "No invented values",
  emergency: "Emergency",
  hand_off: "Hand-off",
  phone_style: "Phone style",
  intent_drift: "Stayed on task",
};
const VERDICT_WORDS: Record<Verdict, string> = { pass: "Pass", fail: "Fail", unknown: "Unsure" };
/** Who rates in the console, until staff sign-in names each tester (catalog.py: "owner"). */
const GRADER = "owner";

/** The Review panel (CHAT.md §12): the rubric for one call, the 5th panel of the rail. It
 * slides in when a test call or a Demo playback ends. Every choice saves at once, as one line
 * of the committed annotations file; nothing in it is required. */
class Review {
  private call: string | null = null;
  /** Guards against an older load that finishes after a newer one. */
  private gen = 0;
  private saved = { label: "", note: "", keep: false };

  constructor(
    private readonly panel: HTMLElement,
    private readonly status: HTMLElement,
    private readonly star: HTMLButtonElement,
    /** The head's title: the call's outcome in words. */
    private readonly outcome: HTMLElement,
    /** The subtitle under it: when the call started. */
    private readonly callLine: HTMLElement,
    private readonly label: HTMLInputElement,
    private readonly note: HTMLTextAreaElement,
    private readonly rows: HTMLElement,
    /** Shows or hides the Review tab; `slide`: the call just ended, so the tab is chosen. */
    private readonly shown: (open: boolean, slide: boolean) => void,
  ) {
    star.addEventListener("click", () => {
      if (!this.call) return;
      const keep = star.getAttribute("aria-pressed") !== "true";
      this.setStar(keep);
      this.saved.keep = keep;
      void this.save({ kind: "keep", value: keep });
    });
    const saveText = (kind: "label" | "note", box: HTMLInputElement | HTMLTextAreaElement) => {
      const text = box.value.trim();
      if (!this.call || text === this.saved[kind]) return;
      this.saved[kind] = text;
      void this.save({ kind, text });
    };
    label.addEventListener("change", () => saveText("label", label));
    note.addEventListener("change", () => saveText("note", note));
  }

  /** The call under review, or null when the panel is closed. */
  get current(): string | null {
    return this.call;
  }

  close(): void {
    this.call = null;
    this.gen += 1;
    this.shown(false, false);
  }

  /** Show the review of `callId`. `slide`: the call just ended, so the pill slides to the
   * Review tab and the rail scrolls to it. */
  async open(callId: string, name: string, slide: boolean): Promise<void> {
    const mine = ++this.gen;
    this.call = callId;
    this.status.textContent = "";
    this.outcome.textContent = name;
    this.callLine.textContent = "";
    this.rows.replaceChildren(emptyNote("Loading the rubric"));
    this.shown(true, slide);
    if (slide) this.panel.scrollIntoView({ block: "nearest", behavior: reducedMotion() ? "auto" : "smooth" });

    let found: CallRow | undefined;
    let dimensions: Record<string, string> = {};
    await catalog.refresh("user");
    if (mine !== this.gen) return;
    const cat = catalog.get().value;
    if (!cat) {
      this.rows.replaceChildren(emptyNote("The agent's dev server is not running"));
      return;
    }
    found = cat.calls.find((c) => c.id === callId);
    dimensions = cat.dimensions;
    if (!found) {
      this.rows.replaceChildren(emptyNote("This call is not in the catalog yet"));
      return;
    }
    this.outcome.textContent = outcomeWords(found.outcome);
    this.callLine.textContent = callTime(found);
    this.saved = { label: found.label ?? "", note: found.note ?? "", keep: found.keep };
    this.label.value = this.saved.label;
    this.note.value = this.saved.note;
    this.setStar(found.keep);
    const latest = (kind: Rating["grader_kind"], dim: string) =>
      found!.ratings.filter((r) => r.grader_kind === kind && r.dimension === dim && (kind !== "human" || r.grader === GRADER)).pop();
    this.rows.replaceChildren(
      ...Object.entries(dimensions).map(([key, about]) =>
        dimensionRow(key, about, latest("human", key), latest("model", key), (r) =>
          this.save({ kind: "rating", grader_kind: "human", grader: GRADER, dimension: key, ...r }),
        ),
      ),
    );
  }

  private setStar(on: boolean): void {
    this.star.setAttribute("aria-pressed", String(on));
    this.star.setAttribute("aria-label", on ? "Starred. Press to remove the star" : "Star this call");
  }

  private async save(a: AnnotationDraft): Promise<void> {
    const call = this.call;
    if (!call) return;
    this.status.textContent = "Saving…";
    try {
      // A write: in click order, in the ledger, and the catalog reads again after it.
      await catalog.act(() => annotate({ ...a, call }));
      if (call === this.call) this.status.textContent = "Saved";
    } catch (e) {
      // The gate's reason names a field, never a value (api.ts › AnnotationRefused).
      if (call === this.call)
        this.status.textContent =
          e instanceof AnnotationRefused
            ? `Not saved: the dev server refused it (${e.reason})`
            : e instanceof AgentDown
              ? `Not saved: ${e.message}`
              : "Not saved";
    }
  }
}

/** One rubric row: the dimension's words and description; a Pass · Fail · Unsure control whose
 * pill slides to the choice; a score of 0 to 5 as five dots; a note. The score and the note
 * wait for a verdict, because a rating needs one (catalog.py `check_annotation`). The model's
 * rating of the same dimension stays hidden until this one has a verdict, so it never leads
 * the tester. */
function dimensionRow(
  key: string,
  about: string,
  human: Rating | undefined,
  model: Rating | undefined,
  save: (r: { verdict: Verdict; score: number | null; note: string | null }) => void,
): HTMLElement {
  let verdict = human && (VERDICTS as readonly string[]).includes(human.verdict) ? (human.verdict as Verdict) : undefined;
  let score: number | null = human?.score ?? null;
  let note = human?.note ?? "";
  const words = DIMENSION_WORDS[key] ?? key.replace(/_/g, " ");

  const row = document.createElement("div");
  row.className = "dim-row";
  const head = document.createElement("div");
  head.className = "dim-head";
  const name = document.createElement("span");
  name.className = "dim-name";
  name.textContent = words;
  const noteToggle = document.createElement("button");
  noteToggle.type = "button";
  noteToggle.className = "dim-note-toggle";
  noteToggle.textContent = "Note";
  noteToggle.setAttribute("aria-expanded", "false");
  head.append(name, noteToggle);
  const aboutLine = document.createElement("p");
  aboutLine.className = "dim-about";
  aboutLine.textContent = about;

  // The verdict: a segmented control with a pill that slides to the choice (the nav pill's motion).
  const seg = document.createElement("div");
  seg.className = "verdict";
  seg.setAttribute("role", "radiogroup");
  seg.setAttribute("aria-label", `${words}: verdict`);
  const pill = document.createElement("span");
  pill.className = "verdict-pill";
  pill.setAttribute("aria-hidden", "true");
  seg.append(pill);
  const choices = VERDICTS.map((v) => {
    const b = document.createElement("button");
    b.type = "button";
    b.setAttribute("role", "radio");
    b.dataset.v = v;
    b.textContent = VERDICT_WORDS[v];
    b.addEventListener("click", () => {
      if (verdict === v) return; // a verdict can change, not go back to none (the file appends)
      verdict = v;
      draw();
      emit();
    });
    seg.append(b);
    return b;
  });

  // The score: five dots, 0 to 5. A press on a dot sets that score; a press on the highest lit
  // dot takes one away, so 0 is reachable.
  const dots = document.createElement("div");
  dots.className = "dots";
  dots.setAttribute("role", "group");
  dots.setAttribute("aria-label", `${words}: score from 0 to 5`);
  const dotButtons = [1, 2, 3, 4, 5].map((n) => {
    const d = document.createElement("button");
    d.type = "button";
    d.className = "dot";
    d.setAttribute("aria-label", `Score ${n}`);
    d.addEventListener("click", () => {
      score = score === n ? n - 1 : n;
      draw();
      emit();
    });
    dots.append(d);
    return d;
  });
  const dotsValue = document.createElement("span");
  dotsValue.className = "dots-value";
  dots.append(dotsValue);

  const controls = document.createElement("div");
  controls.className = "dim-controls";
  controls.append(seg, dots);

  const noteBox = document.createElement("input");
  noteBox.type = "text";
  noteBox.className = "review-input dim-note";
  noteBox.maxLength = 2000;
  noteBox.placeholder = `A note on ${words.toLowerCase()}`;
  noteBox.setAttribute("aria-label", `${words}: note`);
  noteBox.value = note;
  noteBox.hidden = !note;
  noteToggle.setAttribute("aria-expanded", String(!noteBox.hidden));
  noteToggle.addEventListener("click", () => {
    noteBox.hidden = !noteBox.hidden;
    noteToggle.setAttribute("aria-expanded", String(!noteBox.hidden));
    if (!noteBox.hidden) noteBox.focus();
  });
  noteBox.addEventListener("change", () => {
    const text = noteBox.value.trim();
    if (text === note) return;
    note = text;
    emit();
  });

  const modelLine = document.createElement("p");
  modelLine.className = "dim-model";

  function emit(): void {
    if (verdict) save({ verdict, score, note: note || null });
  }
  function draw(): void {
    const i = verdict ? VERDICTS.indexOf(verdict) : -1;
    seg.dataset.verdict = verdict ?? "";
    seg.style.setProperty("--choice", String(Math.max(0, i)));
    choices.forEach((b, j) => b.setAttribute("aria-checked", String(j === i)));
    const ready = verdict !== undefined;
    dotButtons.forEach((d, j) => {
      d.disabled = !ready;
      d.setAttribute("aria-pressed", String(score !== null && j < score));
    });
    dotsValue.textContent = score === null ? "—" : `${score}/5`;
    noteBox.disabled = !ready;
    noteToggle.disabled = !ready;
    // The model's rating, once this one has a verdict.
    modelLine.hidden = !(ready && model);
    if (ready && model) {
      const parts = [`Model (${model.grader}): ${VERDICT_WORDS[model.verdict as Verdict] ?? model.verdict}`];
      if (model.score !== null) parts.push(`${model.score}/5`);
      modelLine.textContent = parts.join(" · ") + (model.note ? ` — ${model.note}` : "");
    }
  }
  draw();
  row.append(head, aboutLine, controls, noteBox, modelLine);
  return row;
}

/** The words for each step (a draft, CHAT.md §11). A step not listed shows its own name. */
const STEP_WORDS: Record<string, string> = {
  greet: "Greeting",
  verify: "Verify",
  menu: "Menu",
  schedule: "Schedule",
  goodbye: "Goodbye",
  hand_off: "Hand-off",
};
/** Which limit each tool spends, by the record's `left` keys (ehr_client.py). */
const TOOL_LIMIT: Record<string, string> = { identify: "identify", resend_code: "resend", check_code: "code" };
/** Global tools that mark the step they ran in. */
const MARK_WORDS: Record<string, string> = {
  emergency: "911 line",
  safety_reminder: "Safety reminder",
  medical_question: "Medical question",
  flag_for_staff: "Staff flag",
};

type StepBlock = {
  name: string;
  at: number;
  end?: number;
  count: number;
  tools: string[];
  everywhere: string[];
  marks: string[];
};

/** The rail's Step panel for one call (CHAT.md §11): a block for each step, stacked in the
 * order the call went; the current one lit, with its tools and the limits left; the steps not
 * reached yet faint below. Made new for each call. */
class StepLog {
  private blocks: StepBlock[] = [];
  private all: string[] = [];
  private closing: string[] = [];
  private left: Record<string, number> = {};
  /** `step` events (schema 5) stamp each move; an older record has only late `node` events. */
  private stamped = false;
  private late = false;
  private ended = false;

  constructor(private readonly box: HTMLElement) {
    box.replaceChildren(emptyNote("No call yet"));
  }

  add(e: CallEvent): void {
    const name = typeof e.name === "string" ? e.name : "";
    if (e.kind === "step" && name) {
      this.stamped = true;
      if (Array.isArray(e.steps)) this.all = e.steps.filter((s): s is string => typeof s === "string");
      if (Array.isArray(e.closing_steps)) this.closing = e.closing_steps.filter((s): s is string => typeof s === "string");
      this.enter(name, e.t_ms, strings(e.tools), strings(e.global_tools));
    } else if (e.kind === "node" && name && !this.stamped) {
      this.late = true;
      this.enter(name, e.t_ms, [], []);
    } else if (e.kind === "ehr" && e.left && typeof e.left === "object") {
      Object.assign(this.left, e.left);
      this.render();
    } else if (e.kind === "tool_call" && MARK_WORDS[name]) {
      this.mark(MARK_WORDS[name]);
    } else if (e.kind === "screen") {
      this.mark("911 line (phrase screen)");
    } else if (e.kind === "call_end") {
      this.ended = true;
      const last = this.blocks[this.blocks.length - 1];
      if (last && last.end === undefined) last.end = e.t_ms;
      this.render();
    }
  }

  private enter(name: string, t: number, tools: string[], everywhere: string[]): void {
    const cur = this.blocks[this.blocks.length - 1];
    if (cur && cur.end === undefined && cur.name === name) {
      // The Flow entered the same step again (find_slots inside schedule): one block, counted.
      cur.count += 1;
      if (tools.length) cur.tools = tools;
    } else {
      if (cur && cur.end === undefined) cur.end = t;
      this.blocks.push({ name, at: t, count: 1, tools, everywhere, marks: [] });
    }
    this.render();
  }

  private mark(word: string): void {
    const cur = this.blocks[this.blocks.length - 1];
    if (!cur) return;
    cur.marks.push(word);
    this.render();
  }

  private render(): void {
    if (!this.blocks.length) return;
    const out: HTMLElement[] = [];
    this.blocks.forEach((b, i) => {
      const current = i === this.blocks.length - 1 && !this.ended;
      out.push(this.block(b, current ? "current" : "passed"));
    });
    // The steps not reached yet, faint, in the Flow's order. The steps that end a call are
    // one block of alternatives, until the call ends one way.
    const seen = new Set(this.blocks.map((b) => b.name));
    for (const s of this.all) {
      if (seen.has(s) || this.closing.includes(s)) continue;
      out.push(this.block({ name: s, at: 0, count: 1, tools: [], everywhere: [], marks: [] }, "ahead"));
    }
    const ends = this.closing.filter((s) => !seen.has(s));
    if (!this.ended && ends.length === this.closing.length && ends.length) {
      out.push(this.block({ name: ends.map(stepWord).join(" or "), at: 0, count: 1, tools: [], everywhere: [], marks: [] }, "ahead", true));
    }
    if (this.late) out.push(emptyNote("An older record: each step is stamped at the model's first reply in it, so its times run late."));
    this.box.replaceChildren(...out);
  }

  private block(b: StepBlock, state: "passed" | "current" | "ahead", worded = false): HTMLElement {
    const el = document.createElement("div");
    el.className = `step-block is-${state}`;
    if (state === "current") el.setAttribute("aria-current", "step");
    const head = document.createElement("div");
    head.className = "step-head";
    const icon = document.createElement("span");
    icon.className = "step-icon";
    icon.setAttribute("aria-hidden", "true");
    if (state === "passed") setHtml(icon, CHECK);
    const name = document.createElement("span");
    name.className = "step-name";
    name.textContent = worded ? b.name : stepWord(b.name);
    head.append(icon, name);
    if (b.count > 1) {
      const n = document.createElement("span");
      n.className = "step-count";
      n.textContent = `×${b.count}`;
      head.append(n);
    }
    if (state !== "ahead") {
      const time = document.createElement("span");
      time.className = "step-time";
      time.textContent =
        b.end === undefined ? `from ${callClock(b.at)}` : `${callClock(b.at)} · ${spanWords(b.end - b.at)}`;
      head.append(time);
    }
    el.append(head);

    if (state === "current" && (b.tools.length || b.everywhere.length)) {
      const facts = document.createElement("ul");
      facts.className = "step-facts";
      const fact = (label: string, value: string) => {
        const li = document.createElement("li");
        const l = document.createElement("span");
        l.className = "bullet-label";
        l.textContent = `${label} `;
        const v = document.createElement("span");
        v.textContent = value;
        li.append(l, v);
        facts.append(li);
      };
      if (b.tools.length) {
        fact(
          "Tools",
          b.tools
            .map((t) => {
              const key = TOOL_LIMIT[t];
              const n = key === undefined ? undefined : this.left[key];
              return typeof n === "number" ? `${t} (${n} left)` : t;
            })
            .join(" · "),
        );
      }
      if (b.everywhere.length) fact("Every step", b.everywhere.join(" · "));
      el.append(facts);
    }
    if (b.marks.length) {
      const marks = document.createElement("ul");
      marks.className = "step-marks";
      for (const m of b.marks) {
        const li = document.createElement("li");
        li.textContent = m;
        marks.append(li);
      }
      el.append(marks);
    }
    return el;
  }
}

const CHECK = `<svg viewBox="0 0 24 24"><path d="M5 12.5l4.5 4.5L19 7.5"/></svg>`;

function stepWord(name: string): string {
  return STEP_WORDS[name] ?? name;
}

function strings(v: unknown): string[] {
  return Array.isArray(v) ? v.filter((s): s is string => typeof s === "string") : [];
}

/** A time on the call's clock: "0:42", "12:05". */
function callClock(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

/** The rail's Tools tab for one call (CHAT.md §9): each line that led to a tool call, as a
 * bubble in the canvas's colors (the caller's line; the agent's before the caller has spoken),
 * and under it each tool it triggered: "→" and the tool's name, then bullets for each argument
 * by name, the time to trigger and the time to confirm. Tools from the same line share its
 * bubble. Made new for each call, so it clears the tab. */
class ToolLog {
  private lastCaller: CallEvent | undefined;
  private lastAgent: CallEvent | undefined;
  /** The bubble the newest tools sit under, and the line it shows. */
  private turn: { from: CallEvent | undefined; el: HTMLElement } | undefined;
  /** Rows waiting for their result, by tool name, oldest first. */
  private waiting = new Map<string, ((e: CallEvent) => void)[]>();

  constructor(private readonly box: HTMLElement) {
    box.replaceChildren(emptyNote("No tool calls yet"));
  }

  caller(e: CallEvent): void {
    this.lastCaller = e;
  }

  agent(e: CallEvent): void {
    this.lastAgent = e;
  }

  call(e: CallEvent): void {
    const name = typeof e.name === "string" ? e.name : "tool";

    // The line that led to this tool: a new bubble when it differs from the last tool's.
    const from = this.lastCaller ?? this.lastAgent;
    if (!this.turn || this.turn.from !== from) {
      const turn = document.createElement("div");
      turn.className = "tool-turn";
      const text = from && typeof from.text === "string" ? from.text : undefined;
      if (text) {
        const bubble = document.createElement("div");
        bubble.className = `bubble tool-bubble tool-bubble--${from === this.lastCaller ? "caller" : "agent"}`;
        bubble.textContent = text;
        turn.append(bubble);
      } else {
        turn.append(emptyNote("Before any line"));
      }
      this.turn = { from, el: turn };
      this.show(turn);
    }

    const row = document.createElement("div");
    row.className = "tool";
    const arrow = document.createElement("span");
    arrow.className = "tool-arrow";
    arrow.setAttribute("aria-hidden", "true");
    // Drawn, not the "→" character: the serif's arrow sits low on the line.
    setHtml(arrow, SEND_ICON);
    const n = document.createElement("p");
    n.className = "tool-name";
    n.textContent = name;

    const facts = document.createElement("ul");
    facts.className = "bullets tool-facts";
    const fact = (label: string, value: string) => {
      const li = document.createElement("li");
      const l = document.createElement("span");
      l.className = "bullet-label";
      l.textContent = `${label}: `;
      const v = document.createElement("span");
      v.textContent = value;
      li.append(l, v);
      facts.append(li);
      return v;
    };
    for (const [key, value] of argList(e.arguments)) fact(key, value);
    // trigger_ms is exact (the record's high-resolution clock, schema 3). An older record has
    // only whole-ms event times, so the span is worked out from them.
    const trigger =
      typeof e.trigger_ms === "number"
        ? e.trigger_ms
        : this.lastCaller
          ? e.t_ms - this.lastCaller.t_ms
          : undefined;
    fact("Time to trigger", trigger === undefined ? "—" : spanWords(trigger));
    const confirm = fact("Time to confirm", "waiting…");

    const queue = this.waiting.get(name) ?? [];
    queue.push((r) => {
      const ms = typeof r.ms === "number" ? spanWords(r.ms) : "—";
      confirm.textContent = r.error ? `${ms} · failed` : ms;
    });
    this.waiting.set(name, queue);

    row.append(arrow, n, facts);
    const b = this.scroller();
    const atEnd = this.atEnd(b);
    this.turn.el.append(row);
    if (atEnd) b.scrollTop = b.scrollHeight;
  }

  result(e: CallEvent): void {
    const name = typeof e.name === "string" ? e.name : "tool";
    this.waiting.get(name)?.shift()?.(e);
  }

  /** The list scrolls in its tab's body (CHAT.md §15). */
  private scroller(): HTMLElement {
    return this.box.closest<HTMLElement>(".tab-body") ?? this.box;
  }

  /** Follow the newest only when the reader is already at the bottom. */
  private atEnd(b: HTMLElement): boolean {
    return b.scrollHeight - b.scrollTop - b.clientHeight < 8;
  }

  private show(node: HTMLElement): void {
    const b = this.scroller();
    const atEnd = this.atEnd(b);
    this.box.querySelector(":scope > .empty")?.remove();
    this.box.append(node);
    if (atEnd) b.scrollTop = b.scrollHeight;
  }
}

/** A tool's arguments, each with its name in words ("reason_kind" → "Reason kind"), in order.
 * A list is joined with commas; a string in a list is quoted, as the caller's words are. Empty
 * values are left out. */
function argList(args: unknown): [string, string][] {
  if (!args || typeof args !== "object") return [];
  const words = (key: string) => {
    const w = key.replace(/_/g, " ");
    return w.charAt(0).toUpperCase() + w.slice(1);
  };
  const value = (v: unknown): string =>
    Array.isArray(v)
      ? v.map((x) => (typeof x === "string" ? `“${x}”` : JSON.stringify(x))).join(", ")
      : typeof v === "string"
        ? v
        : JSON.stringify(v);
  return Object.entries(args as Record<string, unknown>)
    .filter(([, v]) => v !== null && v !== undefined && v !== "" && !(Array.isArray(v) && v.length === 0))
    .map(([k, v]) => [words(k), value(v)]);
}

/** A span: under a second in ms to a tenth ("862.3 ms"); from a second, in seconds to the
 * millisecond ("1.420 s"), so no precision is lost. */
function spanWords(ms: number): string {
  return ms < 1000 ? `${ms.toFixed(1)} ms` : `${(ms / 1000).toFixed(3)} s`;
}

function emptyNote(text: string): HTMLElement {
  const p = document.createElement("p");
  p.className = "empty";
  p.textContent = text;
  return p;
}

/** The agent's line facts (CHAT.md §4, a draft): the time to the first word of its first live
 * request, or "replayed" when the replay cache answered. Speech times come with the voice. */
function agentFacts(llm: CallEvent[]): string | undefined {
  if (!llm.length) return undefined;
  const live = llm.find((e) => !e.hit);
  if (!live) return "replayed";
  const ms = (live.first_text_ms ?? live.total_ms) as number | undefined;
  return typeof ms === "number" ? `text ${(ms / 1000).toFixed(1)} s` : undefined;
}

/** The Resources tab (CHAT.md §17; the owner, 2026-11-14): two columns, Anthropic | Cartesia,
 * in three blocks split by lines: what is left this cycle (a bar and "N of M left"), the shown
 * call's use (Claude in cents, Cartesia in credits), and the cycle's facts. Then the notes.
 * Counts and dates only, never a value from a call. */
function resourceRows(
  a: AnthropicCycle,
  c: CartesiaCycle,
  call: CallUse | null,
  keyed: boolean,
): HTMLElement[] {
  const n = (x: number) => Math.round(x).toLocaleString("en-US");
  const usd = (x: number) =>
    `$${x.toLocaleString("en-US", { minimumFractionDigits: Number.isInteger(x) ? 0 : 2, maximumFractionDigits: 2 })}`;
  const cents = (x: number) => {
    const c100 = x * 100;
    return `${c100.toLocaleString("en-US", { maximumFractionDigits: c100 < 10 ? 2 : 1 })}¢`;
  };
  const minutes = (s: number) => {
    const whole = Math.round(s);
    return `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, "0")}`;
  };
  const day = (iso: string) => {
    const [y, m, d] = iso.split("-").map(Number);
    return new Date(y, m - 1, d).toLocaleDateString("en-US", { month: "short", day: "numeric" });
  };
  const plural = (k: number, one: string, many: string) => `${k} ${k === 1 ? one : many}`;
  const text = (tag: string, cls: string, words: string) => {
    const e = document.createElement(tag);
    e.className = cls;
    e.textContent = words;
    return e;
  };
  // The bar fills with the share left; aria says the same in numbers.
  const meter = (label: string, left: number, total: number) => {
    const m = document.createElement("div");
    m.className = "res-meter";
    m.setAttribute("role", "meter");
    m.setAttribute("aria-label", label);
    m.setAttribute("aria-valuemin", "0");
    m.setAttribute("aria-valuemax", String(total));
    m.setAttribute("aria-valuenow", String(Math.max(0, Math.min(left, total))));
    const fill = document.createElement("span");
    fill.className = "res-fill";
    fill.style.setProperty("--share", String(total > 0 ? Math.max(0, Math.min(1, left / total)) : 0));
    m.append(fill);
    return m;
  };
  const cell = (side: "anthropic" | "cartesia", ...parts: HTMLElement[]) => {
    const e = document.createElement("div");
    e.className = `res-cell res-${side}`;
    e.append(...parts);
    return e;
  };

  const aBudget = a.budget - a.offset;
  const cPlan = c.plan - c.offset;
  const top = [
    cell(
      "anthropic",
      text("h2", "res-head", "Anthropic"),
      meter("Claude dollars left this cycle", a.left, aBudget),
      text("p", "res-left", `${usd(Math.max(0, a.left))} of ${usd(aBudget)} left`),
    ),
    cell(
      "cartesia",
      text("h2", "res-head", "Cartesia"),
      meter("Cartesia credits left this cycle", c.left, cPlan),
      text("p", "res-left", `${n(c.left)} of ${n(cPlan)} left`),
    ),
  ];

  const callHead = call?.live ? "This call so far" : "This call";
  const middle = [
    cell(
      "anthropic",
      text("p", "res-term", callHead),
      text("p", "res-value", call ? cents(call.usd) : "No call yet"),
    ),
    cell(
      "cartesia",
      text("p", "res-term", callHead),
      text(
        "p",
        "res-value",
        !call ? "No call yet" : call.credits === null ? "Text call" : `${n(call.credits)} credits`,
      ),
    ),
  ];

  const bottom = [
    cell(
      "anthropic",
      text("p", "res-fact", plural(a.calls, "call this cycle", "calls this cycle")),
      text("p", "res-fact", `Renews ${day(a.renews)}`),
    ),
    cell(
      "cartesia",
      text("p", "res-fact", plural(c.calls, "voice call this cycle", "voice calls this cycle")),
      text(
        "p",
        "res-fact",
        `Ink ${minutes(c.ink_s)} · Sonic ${n(c.sonic_chars)} ${c.sonic_chars === 1 ? "character" : "characters"}`,
      ),
      text("p", "res-fact", `Renews ${day(c.renews)}`),
    ),
  ];

  const grid = document.createElement("div");
  grid.className = "res-grid";
  for (const [i, block] of [top, middle, bottom].entries()) {
    for (const e of block) {
      e.classList.toggle("res-after-line", i > 0);
      grid.append(e);
    }
  }

  const limits = keyed
    ? `A voice test call ends after ${c.limits.idle_s} s with no speech, or at ${minutes(c.limits.max_s)}.`
    : "No Cartesia key in .env: voice calls are off.";
  const notes = text(
    "p",
    "res-note",
    `${limits} Counted from this machine's call records; not read from Anthropic or Cartesia. Their dashboards have the full count.`,
  );
  return [grid, notes];
}

function endNote(reason: string): HTMLElement {
  const p = document.createElement("p");
  p.className = "empty";
  p.textContent = reason ? `Call ended: ${reason}` : "Call ended";
  return p;
}

/** One recorded call in Demo's list (CHAT.md §16): a row of two buttons. The wide one chooses
 * the call: its main line is the label, or the outcome when it has none, starred calls marked
 * "★"; under it, in small text, when it started, its outcome (when the label took the main
 * line), its scenario and its length. The round one at the right plays the call at once. */
function callRow(c: CallRow): HTMLElement {
  const row = document.createElement("div");
  row.className = "call-row";
  row.dataset.id = c.id;
  const pick = document.createElement("button");
  pick.type = "button";
  pick.className = "call-pick";
  const playOne = document.createElement("button");
  playOne.type = "button";
  playOne.className = "call-play";
  setHtml(playOne, PLAY_ICON);
  const name = document.createElement("span");
  name.className = "call-name";
  name.textContent = `${c.keep ? "★ " : ""}${c.label ?? outcomeWords(c.outcome)}`;
  const meta = document.createElement("span");
  meta.className = "call-meta";
  meta.textContent = [callTime(c), c.label ? outcomeWords(c.outcome) : null, c.scenario, callClock(c.duration_ms)]
    .filter(Boolean)
    .join(" · ");
  pick.append(name, meta);
  const words = name.textContent;
  playOne.setAttribute("aria-label", `Play ${words}`);
  playOne.title = "Play this call";
  row.append(pick, playOne);
  return row;
}

/** One live call in PHI's Caller tab (docs/ops/DEMO.md §3.6), newest first: its label, or who
 * started it; then when, the seat, the EHR, the step it is on and the caller's turns so far. The
 * row being watched carries the chosen row's look (Demo's list). Analysts here have full
 * clearance, so the starter's email shows as the dev server sends it. */
function liveRow(info: LiveInfo, events: CallEvent[], watched: boolean): HTMLElement {
  const row = document.createElement("div");
  row.className = "call-row";
  row.dataset.id = info.call;
  row.classList.toggle("is-chosen", watched);
  const pick = document.createElement("button");
  pick.type = "button";
  pick.className = "call-pick";
  pick.setAttribute("aria-pressed", String(watched));
  const who = info.by === "owner" ? "Owner" : info.by;
  const name = document.createElement("span");
  name.className = "call-name";
  name.textContent = info.label ?? `${who}'s call`;
  const step = [...events].reverse().find((e) => e.kind === "step");
  const turns = events.filter((e) => e.kind === "caller").length;
  const meta = document.createElement("span");
  meta.className = "call-meta";
  meta.textContent = [
    info.started.slice(11, 16),
    info.label ? who : null,
    SEAT_WORD[info.seat],
    EHR_WORD[info.ehr],
    step ? stepWord(String(step.name)) : null,
    `${turns} ${turns === 1 ? "turn" : "turns"}`,
  ]
    .filter(Boolean)
    .join(" · ");
  pick.append(name, meta);
  pick.title = watched ? "Watching this call" : "Watch this call";
  row.append(pick);
  return row;
}

/** When a call started, as the catalog stores it ("11-12 18:11"); its id when it has no start. */
function callTime(c: CallRow): string {
  return c.started ? c.started.replace("T", " ").slice(5, 16) : c.id;
}

/** A call's outcome as a title: "handed_off" → "Handed off". */
function outcomeWords(outcome: string): string {
  const words = outcome.replace(/_/g, " ");
  return words.charAt(0).toUpperCase() + words.slice(1);
}

/** A rail panel's tab bar (CHAT.md §15): a horizontal nav. The accent pill slides to the chosen
 * tab, as the big nav's does; Left and Right move through the tabs; the panel remembers its
 * last tab. A tab can be hidden (Review, until a call ends); when the chosen tab hides, the one
 * chosen before it comes back. */
class Tabs {
  private readonly bar: HTMLElement;
  private readonly pill: HTMLElement;
  private readonly tabs: HTMLButtonElement[];
  private readonly bodies: HTMLElement[];
  private current = "";
  private previous = "";

  constructor(
    root: HTMLElement,
    private readonly key: string,
  ) {
    this.bar = root.querySelector<HTMLElement>(".tabs")!;
    this.pill = this.bar.querySelector<HTMLElement>(".tab-pill")!;
    this.tabs = [...this.bar.querySelectorAll<HTMLButtonElement>(".tab")];
    this.bodies = [...root.querySelectorAll<HTMLElement>(".tab-body")];
    this.tabs.forEach((t) => {
      t.id = `${root.id}-tab-${t.dataset.tab}`;
      const body = this.bodies.find((b) => b.dataset.tab === t.dataset.tab)!;
      body.id ||= `${root.id}-${t.dataset.tab}`;
      t.setAttribute("aria-controls", body.id);
      body.setAttribute("aria-labelledby", t.id);
    });
    this.bar.addEventListener("click", (e) => {
      const t = (e.target as HTMLElement).closest<HTMLButtonElement>(".tab");
      if (t) this.pick(t.dataset.tab!, true);
    });
    this.bar.addEventListener("keydown", (e) => {
      if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
      const shown = this.tabs.filter((t) => !t.hidden);
      const at = shown.indexOf(document.activeElement as HTMLButtonElement);
      if (at < 0) return;
      e.preventDefault();
      const next = shown[(at + (e.key === "ArrowRight" ? 1 : -1) + shown.length) % shown.length];
      next.focus();
      this.pick(next.dataset.tab!, true);
    });
    // Also places the pill the first time the page is shown, when the bar first has a size.
    new ResizeObserver(() => this.place(false)).observe(this.bar);

    let saved: string | null = null;
    try {
      saved = localStorage.getItem(key);
    } catch {
      /* storage blocked: the first tab */
    }
    const first = this.tabs.find((t) => t.dataset.tab === saved && !t.hidden) ?? this.tabs[0];
    this.pick(first.dataset.tab!, false);
  }

  /** Choose tab `id`. `animate`: the pill slides and the body fades in. */
  pick(id: string, animate: boolean): void {
    if (id === this.current) return;
    if (this.current) this.previous = this.current;
    this.current = id;
    this.tabs.forEach((t) => {
      const on = t.dataset.tab === id;
      t.setAttribute("aria-selected", String(on));
      t.tabIndex = on ? 0 : -1; // one tab stop for the bar; the arrows move inside it
    });
    this.bodies.forEach((b) => {
      const on = b.dataset.tab === id;
      b.hidden = !on;
      b.classList.remove("entering");
      if (on && animate && !reducedMotion()) {
        void b.offsetWidth; // restart the fade
        b.classList.add("entering");
      }
    });
    this.place(animate);
    try {
      localStorage.setItem(this.key, id);
    } catch {
      /* the tab holds for this visit only */
    }
  }

  /** Show or hide tab `id`. A hidden tab that was chosen gives way to the one before it. */
  show(id: string, shown: boolean): void {
    const t = this.tabs.find((x) => x.dataset.tab === id)!;
    t.hidden = !shown;
    if (!shown && this.current === id) {
      const back = this.tabs.find((x) => x.dataset.tab === this.previous && !x.hidden) ?? this.tabs[0];
      this.pick(back.dataset.tab!, true);
    } else {
      this.place(false);
    }
  }

  /** Moves the pill onto the chosen tab: after a pick, and when a tab's word changes. */
  place(animate: boolean): void {
    placePill(this.pill, this.tabs.find((t) => t.dataset.tab === this.current), animate);
  }
}

/** The rail's top panel: the Review tab shows when a call ends (and is chosen then), and
 * hides when the next call starts. */
function reviewShown(tabs: Tabs) {
  return (open: boolean, slide: boolean) => {
    tabs.show("review", open);
    if (open && slide) tabs.pick("review", true);
  };
}

const CHEVRON = `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 9l6 6 6-6"/></svg>`;

/** Bullets in the rail's style (`.bullets`): each a fainter label, then its value. */
function bulletList(className: string, items: [string, string][]): HTMLUListElement {
  const ul = document.createElement("ul");
  ul.className = `bullets ${className}`;
  for (const [label, value] of items) {
    const li = document.createElement("li");
    const l = document.createElement("span");
    l.className = "bullet-label";
    l.textContent = `${label}: `;
    const v = document.createElement("span");
    v.textContent = value;
    li.append(l, v);
    ul.append(li);
  }
  return ul;
}

/** The sandbox patients for the rail (CHAT.md §8.2): each one's name as the main line; under
 * it, bullets for the birth date and the phone number; then the visits as a dropdown that one
 * press opens and the next closes. */
function sandboxRows(s: Sandbox): HTMLElement[] {
  // A dev server older than this page sends no `more`: read it as none left out.
  const more = s.more ?? {};
  return s.patients.map((p, i) => {
    const row = document.createElement("div");
    row.className = "patient";
    const name = document.createElement("p");
    name.className = "patient-name";
    name.textContent = `${p.given} ${p.family}`;
    row.append(
      name,
      bulletList("patient-facts", [
        ["Born", longDay(p.birth_date)],
        ["Phone", phoneWords(p.phone)],
      ]),
    );

    const visits = s.visits.filter((v) => v.patient === p.id).sort((a, b) => a.starts_at.localeCompare(b.starts_at));
    if (!visits.length) {
      // The dropdown's place and shape, with nothing to open.
      const none = document.createElement("p");
      none.className = "visits-toggle visits-none";
      none.textContent = "No visits";
      row.append(none);
      return row;
    }
    const listId = `sandbox-visits-${i}`;
    const toggle = document.createElement("button");
    toggle.type = "button";
    toggle.className = "visits-toggle";
    toggle.setAttribute("aria-expanded", "false");
    toggle.setAttribute("aria-controls", listId);
    setHtml(toggle, `<span></span>${CHEVRON}`);
    const count = visits.length + (more[p.id] ?? 0);
    toggle.querySelector("span")!.textContent = count === 1 ? "1 visit" : `${count} visits`;
    const fold = document.createElement("div");
    fold.className = "visits";
    fold.id = listId;
    const inner = document.createElement("div");
    inner.className = "visits-inner";
    const ul = document.createElement("ul");
    ul.className = "bullets";
    for (const v of visits) {
      const li = document.createElement("li");
      const past = v.status && v.status !== "booked" ? v.status : null;
      li.textContent = [visitTime(v.starts_at), `${v.minutes} min`, past, v.visit_type, v.reason, v.clinician, v.place ?? s.place]
        .filter(Boolean)
        .join(" · ");
      ul.append(li);
    }
    const rest = more[p.id] ?? 0;
    if (rest > 0) {
      const li = document.createElement("li");
      li.className = "visits-more";
      li.textContent = rest === 1 ? "and 1 more" : `and ${rest} more`;
      ul.append(li);
    }
    inner.append(ul);
    fold.append(inner);
    toggle.addEventListener("click", () => {
      const open = toggle.getAttribute("aria-expanded") !== "true";
      toggle.setAttribute("aria-expanded", String(open));
      row.classList.toggle("open", open);
    });
    row.append(toggle, fold);
    return row;
  });
}

/** The open code button's card: the newest text first, in full; older ones marked replaced,
 * because a new code stops the old one working (ehr/src/service.rs › resend_code). */
function textCards(list: { to: string; body: string }[], s: Sandbox | undefined): HTMLElement[] {
  const head = document.createElement("h2");
  head.textContent = list.length === 1 ? "Text message" : "Text messages";
  const cards = [...list].reverse().map((t, i) => {
    const card = document.createElement("div");
    card.className = i === 0 ? "text-card" : "text-card replaced";
    const who = s?.patients.find((p) => p.phone === t.to);
    const meta = document.createElement("p");
    meta.className = "text-meta";
    meta.textContent = [`To ${phoneWords(t.to)}`, who && `${who.given} ${who.family}`, i > 0 && "Replaced"]
      .filter(Boolean)
      .join(" · ");
    const body = document.createElement("p");
    body.className = "text-body";
    body.textContent = t.body;
    card.append(meta, body);
    return card;
  });
  return [head, ...cards];
}

/** "+15555550101" → "(555) 555-0101"; any other number as it came. */
function phoneWords(e164: string): string {
  const m = /^\+1(\d{3})(\d{3})(\d{4})$/.exec(e164);
  return m ? `(${m[1]}) ${m[2]}-${m[3]}` : e164;
}

/** "1984-03-12" → "March 12, 1984". Read as UTC, so no time zone moves the day. */
function longDay(day: string): string {
  const [y, m, d] = day.split("-").map(Number);
  if (!y || !m || !d) return day;
  return new Date(Date.UTC(y, m - 1, d)).toLocaleDateString("en-US", {
    timeZone: "UTC",
    year: "numeric",
    month: "long",
    day: "numeric",
  });
}

/** "2026-11-10T10:00" (local time at the place) → "Tue, Nov 10, 2026, 10:00 AM". */
function visitTime(at: string): string {
  const m = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})$/.exec(at);
  if (!m) return at;
  const [y, mo, d, h, mi] = m.slice(1).map(Number);
  return new Date(Date.UTC(y, mo - 1, d, h, mi)).toLocaleString("en-US", {
    timeZone: "UTC",
    weekday: "short",
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

/** One line of the call: the bubble, and its facts row when there are facts to show. */
function line(who: "agent" | "caller", text: string, facts?: string): HTMLElement {
  const wrap = document.createElement("div");
  wrap.className = `line line--${who}`;
  const bubble = document.createElement("div");
  bubble.className = "bubble";
  bubble.textContent = text;
  wrap.append(bubble);
  if (facts) {
    const f = document.createElement("div");
    f.className = "facts";
    f.textContent = facts;
    wrap.append(f);
  }
  return wrap;
}
