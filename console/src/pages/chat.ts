// Chat (CHAT.md): one call in a chat view, in three modes, beside the rail of four panels.
// - PHI: watch a call. Read only: no controls, no text bar.
// - Test: the user is the caller. The help box (the sandbox patients), Start | End, the text bar.
// - Demo: a recorded call plays back as if live. Play | Stop, the call picker and the speed.
// The page is the UI only for now: the connection to the agent is the next step (CHAT.md §4),
// so a test call takes the caller's lines and nothing answers yet.
import { reducedMotion, tokenMs } from "../tokens";
import { el, type Page } from "./page";

type Mode = "phi" | "test" | "demo";
const MODES: Mode[] = ["phi", "test", "demo"];
const MODE_WORD: Record<Mode, string> = { phi: "PHI", test: "Test", demo: "Demo" };
const MODE_ABOUT: Record<Mode, string> = {
  phi: "Watch a call. Read only",
  test: "You are the caller",
  demo: "A recorded call plays back as if live",
};
const MODE_KEY = "deets.console.chat.mode";

// The sandbox patients, all synthetic. Keep in step with ehr/src/verify.rs (the same list as
// agent/harness/typed_call.py). The dev server will send this list when the connection lands.
const SANDBOX = [
  ["Jordan Ellis", "Born March 12, 1984. Has a visit on November 10."],
  ["Priya Raman", "Born November 30, 1971. Has no visits."],
  ["Casey Morgan", "Born June 5, 1990. Shares the birth date with Casey Morgen, so neither verifies."],
];
const SANDBOX_CODE = "246810";

const SEND_ICON = `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12h13M12 5l7 7-7 7"/></svg>`;

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
  return {
    id: "chat",
    title: "Chat",
    shown: () => requestAnimationFrame(() => shownHook()),
    build() {
      const page = el(`
        <section class="page">
          <div class="split-page">
            <section class="stage glass chat" aria-labelledby="chat-title" data-mode="test">
              <header class="chat-head">
                <h1 id="chat-title">Chat</h1>
                <button class="mode-toggle" id="chat-mode" type="button"><span class="mode-word"></span></button>
              </header>

              <div class="chat-canvas">
                <div class="chat-lines" data-for="phi" role="log" aria-live="polite" aria-label="The call">
                  <p class="empty">No live call to watch</p>
                </div>
                <div class="chat-lines" data-for="test" role="log" aria-live="polite" aria-label="The call">
                  <p class="empty">Press Start to begin a test call</p>
                </div>
                <div class="chat-lines" data-for="demo" role="log" aria-live="polite" aria-label="The call">
                  <p class="empty">Pick a recorded call, then press Play</p>
                </div>

                <div class="chat-controls">
                  <div class="help" id="chat-help">
                    <div class="help-card" id="chat-help-card" role="region" aria-label="Sandbox patients">
                      <div class="help-inner"><div class="help-body">
                        <h2>Sandbox patients</h2>
                        <dl>${SANDBOX.map(([n, about]) => `<dt>${n}</dt><dd>${about}</dd>`).join("")}</dl>
                        <p>The text code is always <strong>${SANDBOX_CODE}</strong>. No text is sent.</p>
                        <p class="note">All of these are synthetic.</p>
                      </div></div>
                    </div>
                    <button class="control" data-wave id="chat-help-button" type="button" aria-expanded="false"
                      aria-controls="chat-help-card">Sandbox patients</button>
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

              <div class="chat-bar bar-demo">
                <select class="control picker" data-wave id="demo-pick" aria-label="Recorded call" disabled>
                  <option>No recorded calls yet</option>
                </select>
                <div class="split" data-wave role="group" aria-label="Speed">
                  <button class="split-plain" type="button" data-speed="1" aria-pressed="true"><span>1×</span></button>
                  <button class="split-plain" type="button" data-speed="2" aria-pressed="false"><span>2×</span></button>
                </div>
              </div>
            </section>

            <aside class="rail" aria-label="Chat details">
              ${["Timeline", "Caller", "Step", "Tools"]
                .map(
                  (t) =>
                    `<section class="panel glass" aria-labelledby="chat-${t.toLowerCase()}">` +
                    `<h2 id="chat-${t.toLowerCase()}">${t}</h2><p class="empty">Empty for now</p></section>`,
                )
                .join("")}
            </aside>
          </div>
        </section>`);

      const $ = <T extends HTMLElement>(id: string) => page.querySelector<T>(`#${id}`)!;
      const chat = page.querySelector<HTMLElement>(".chat")!;
      const lines = (m: Mode) => page.querySelector<HTMLElement>(`.chat-lines[data-for="${m}"]`)!;

      // ---- the mode toggle: PHI → Test → Demo → PHI, the word turning over
      const toggle = $<HTMLButtonElement>("chat-mode");
      const word = toggle.querySelector<HTMLElement>(".mode-word")!;
      let mode = savedMode();
      const showMode = () => {
        chat.dataset.mode = mode;
        word.textContent = MODE_WORD[mode];
        const next = MODES[(MODES.indexOf(mode) + 1) % MODES.length];
        toggle.setAttribute("aria-label", `Mode: ${MODE_WORD[mode]}. ${MODE_ABOUT[mode]}. Press for ${MODE_WORD[next]}`);
        toggle.title = `${MODE_ABOUT[mode]}. Press for ${MODE_WORD[next]}`;
        MODES.forEach((m) => (lines(m).hidden = m !== mode));
        if (mode !== "test") closeHelp();
      };
      // ---- the wave (chat.css › the wave): the visible controls come in one after another,
      // top row first, left to right, as one wave.
      let waveTimer = 0;
      const waveIn = () => {
        if (reducedMotion()) return;
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

      let flipping = 0;
      toggle.addEventListener("click", () => {
        mode = MODES[(MODES.indexOf(mode) + 1) % MODES.length];
        try {
          localStorage.setItem(MODE_KEY, mode);
        } catch {
          /* the mode holds for this visit only */
        }
        if (reducedMotion()) return showMode();
        // The old word turns away while the old controls fade out together; at the halfway
        // point the mode swaps, the new word turns in, and the new controls come in as a wave.
        const mine = ++flipping;
        chat.classList.remove("wave-in");
        chat.classList.add("wave-out");
        toggle.classList.add("flip-out");
        window.setTimeout(() => {
          if (mine !== flipping) return;
          toggle.classList.remove("flip-out");
          toggle.classList.add("flip-in");
          showMode();
          chat.classList.remove("wave-out");
          waveIn();
          requestAnimationFrame(() => requestAnimationFrame(() => toggle.classList.remove("flip-in")));
        }, Math.max(tokenMs("--dur-flip") / 2, tokenMs("--dur-wave-out")));
      });
      shownHook = waveIn;

      // ---- the help box: one press opens it, the next closes it
      const help = $("chat-help");
      const helpButton = $<HTMLButtonElement>("chat-help-button");
      const closeHelp = () => {
        help.classList.remove("open");
        helpButton.setAttribute("aria-expanded", "false");
      };
      helpButton.addEventListener("click", () => {
        const open = !help.classList.contains("open");
        help.classList.toggle("open", open);
        helpButton.setAttribute("aria-expanded", String(open));
      });

      // ---- a test call: Start | End, with End asking "Confirm?" first
      const start = $<HTMLButtonElement>("call-start");
      const end = $<HTMLButtonElement>("call-end");
      const endWord = end.querySelector<HTMLElement>('[data-word="end"]')!;
      const confirmWord = end.querySelector<HTMLElement>('[data-word="confirm"]')!;
      const input = $<HTMLInputElement>("chat-input");
      const send = $<HTMLButtonElement>("chat-send");
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
        setConfirm(false);
        start.disabled = on;
        end.disabled = !on;
        input.disabled = !on;
        send.disabled = !on;
        input.placeholder = on ? "Type as the caller, then press Enter" : "Start a call to type as the caller";
      };
      const note = (text: string) => {
        const p = document.createElement("p");
        p.className = "empty";
        p.textContent = text;
        return p;
      };

      start.addEventListener("click", () => {
        testLines.replaceChildren(note("Call started. The agent is not connected yet."));
        setLive(true);
        input.focus();
      });
      end.addEventListener("click", () => {
        if (!live) return;
        if (confirmWord.hidden) return setConfirm(true);
        setLive(false);
        testLines.append(note("Call ended"));
        start.focus();
      });
      end.addEventListener("keydown", (e) => {
        if (e.key === "Escape" && !confirmWord.hidden) setConfirm(false);
      });

      $<HTMLFormElement>("chat-form").addEventListener("submit", (e) => {
        e.preventDefault();
        const text = input.value.trim();
        if (!live || !text) return;
        testLines.querySelector(".empty")?.remove();
        testLines.append(line("caller", text));
        input.value = "";
        testLines.scrollTop = testLines.scrollHeight;
      });

      // ---- Demo's speed: one of the pair is chosen
      const speeds = [...page.querySelectorAll<HTMLButtonElement>("[data-speed]")];
      speeds.forEach((b) =>
        b.addEventListener("click", () => speeds.forEach((x) => x.setAttribute("aria-pressed", String(x === b)))),
      );

      showMode();
      return page;
    },
  };
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
