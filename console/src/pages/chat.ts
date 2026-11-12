// Chat (CHAT.md): one call in a chat view, in three modes, beside the rail of four panels.
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
  catalog,
  callEvents,
  sandbox,
  VERDICTS,
  watch,
  type AnnotationDraft,
  type CallEvent,
  type CallRow,
  type Rating,
  type Sandbox,
  type Verdict,
} from "../agent/api";
import { TextSeat } from "../agent/transport";
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

const STAR_ICON = `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 3.5l2.6 5.3 5.9.9-4.3 4.1 1 5.8-5.2-2.7-5.2 2.7 1-5.8-4.3-4.1 5.9-.9z"/></svg>`;
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
                  <div class="help code" id="chat-code" hidden>
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
              ${emptyPanel("Timeline")}
              <section class="panel glass panel-sandbox" aria-labelledby="chat-sandbox">
                <h2 id="chat-sandbox">Sandbox patients</h2>
                <div class="sandbox" id="sandbox-list"><p class="empty">Loading</p></div>
              </section>
              ${emptyPanel("Caller")}
              <section class="panel glass panel-step" aria-labelledby="chat-step">
                <h2 id="chat-step">Step</h2>
                ${MODES.map(
                  (m) => `<div class="steps-log" data-for="${m}" aria-label="The call's steps">` +
                    `<p class="empty">No call yet</p></div>`,
                ).join("")}
              </section>
              <section class="panel glass panel-tools" aria-labelledby="chat-tools">
                <h2 id="chat-tools">Tools</h2>
                ${MODES.map(
                  (m) => `<div class="tools-log" data-for="${m}" role="log" aria-label="Tool calls">` +
                    `<p class="empty">No tool calls yet</p></div>`,
                ).join("")}
              </section>
              <section class="panel glass panel-review" id="chat-review" aria-labelledby="chat-review-title" hidden>
                <div class="review-head">
                  <h2 id="chat-review-title">Review</h2>
                  <span class="review-status" id="review-status" role="status"></span>
                  <button class="review-star" id="review-star" type="button" aria-pressed="false"
                    aria-label="Star this call">${STAR_ICON}</button>
                </div>
                <p class="review-call" id="review-call"></p>
                <input class="review-input" id="review-label" type="text" maxlength="200" autocomplete="off"
                  placeholder="Label" aria-label="A label for this call" />
                <textarea class="review-input" id="review-note" rows="2" maxlength="2000"
                  placeholder="Note" aria-label="A note on this call"></textarea>
                <div class="review-rows" id="review-rows"></div>
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
        splitPage.dataset.mode = mode;
        MODES.forEach((m) => {
          lines(m).hidden = m !== mode;
          toolsBox(m).hidden = m !== mode;
          stepsBox(m).hidden = m !== mode;
        });
        closeCode();
        renderCode(false);
        showReview(false);
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

      // ---- the Review panel (CHAT.md §12): the call each mode last finished, Test's from its
      // own call, Demo's from the playback that ran to its end. PHI rates nothing.
      const review = new Review(
        $("chat-review"),
        $("review-status"),
        $<HTMLButtonElement>("review-star"),
        $("review-call"),
        $<HTMLInputElement>("review-label"),
        $<HTMLTextAreaElement>("review-note"),
        $("review-rows"),
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

      // ---- the sandbox patients (the rail, Test and Demo): from the dev server, which reads
      // the same file ehr's fakes read (ehr/fixtures/sandbox.json)
      const sandboxList = $("sandbox-list");
      let box: Sandbox | undefined;
      const loadSandbox = async () => {
        try {
          box = await sandbox();
          sandboxList.replaceChildren(...sandboxRows(box));
          renderCode(false);
        } catch {
          sandboxList.replaceChildren(note("The agent's dev server is not running"));
        }
      };

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
        void client.sendText(latest.code, { run_immediately: true, audio_response: false }).catch(() => {});
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
        renderCode(false);
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
      let myCall: string | null = null;
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
        seeking = true;
        client = new PipecatClient({
          transport: new TextSeat(),
          enableMic: false,
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
          start.disabled = false;
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
        void client.sendText(text, { run_immediately: true, audio_response: false }).catch(() => {});
        input.value = "";
      });

      // ---- the watch stream: a live call's events. Test draws its own call; PHI draws any.
      const phiLines = lines("phi");
      let phiView: CallView | undefined;
      let phiCall: string | null = null;
      const phiIdle = () => phiLines.replaceChildren(note("No live call to watch"));
      watch(
        (m) => {
          if (m.type === "hello" || m.type === "started") {
            const id = m.type === "hello" ? m.live : m.call;
            if (id) {
              phiCall = id;
              // PHI shows every field in dev (synthetic), as CHAT.md §2.4 decided; roles gate it later.
              phiView = new CallView(phiLines, panels("phi"));
              phiLines.replaceChildren();
              // The dev server runs one test call at a time, so the call that starts while this
              // page is calling is this page's call.
              if (m.type === "started" && seeking) {
                myCall = id;
                seeking = false;
              }
            } else phiIdle();
            return;
          }
          if (m.type === "event") {
            if (m.call === phiCall) phiView?.add(m.event);
            if (m.call === myCall) testView?.add(m.event);
            return;
          }
          if (m.type === "ended") {
            if (m.call === phiCall) {
              phiCall = null;
              phiLines.append(note("The call ended"));
            }
            if (m.call === myCall && live) {
              setLive(false);
              void hangUp();
            }
            // The dev server adds the call to the catalog before it says "ended".
            if (m.call === myCall) finished("test", m.call);
            void loadCatalog();
          }
        },
        (up) => {
          if (!up && !phiCall) phiLines.replaceChildren(note("The agent's dev server is not running"));
          else if (up && !phiCall) phiIdle();
          if (up && !box) void loadSandbox();
        },
      );

      // ---- Demo: a recorded call plays back as if live, from the catalog
      const pick = $<HTMLSelectElement>("demo-pick");
      const play = $<HTMLButtonElement>("demo-play");
      const stop = $<HTMLButtonElement>("demo-stop");
      const demoLines = lines("demo");
      const speeds = [...page.querySelectorAll<HTMLButtonElement>("[data-speed]")];
      let speed = 1;
      speeds.forEach((b) =>
        b.addEventListener("click", () => {
          speed = Number(b.dataset.speed) || 1;
          speeds.forEach((x) => x.setAttribute("aria-pressed", String(x === b)));
        }),
      );
      let timers: number[] = [];
      const stopDemo = () => {
        timers.forEach((t) => window.clearTimeout(t));
        timers = [];
        stop.disabled = true;
        play.disabled = pick.disabled;
      };
      async function loadCatalog() {
        let calls: CallRow[];
        try {
          calls = (await catalog()).calls;
        } catch {
          pick.replaceChildren(new Option("The agent's dev server is not running"));
          pick.disabled = play.disabled = true;
          return;
        }
        const chosen = pick.value;
        pick.replaceChildren(...calls.map((c) => new Option(callName(c), c.id)));
        if (!calls.length) pick.append(new Option("No recorded calls yet"));
        pick.disabled = !calls.length;
        if (calls.some((c) => c.id === chosen)) pick.value = chosen;
        play.disabled = pick.disabled || timers.length > 0;
      }
      play.addEventListener("click", async () => {
        if (pick.disabled) return;
        stopDemo();
        play.disabled = true;
        stop.disabled = false;
        let events: CallEvent[];
        try {
          events = await callEvents(pick.value);
        } catch {
          demoLines.replaceChildren(note("That call could not be read"));
          return stopDemo();
        }
        demoLines.replaceChildren();
        clearTexts("demo");
        unreview("demo");
        const id = pick.value;
        const view = new CallView(demoLines, { onText: (e) => addText("demo", e), ...panels("demo") });
        const last = events.reduce((m, e) => Math.max(m, e.t_ms), 0);
        for (const e of events) timers.push(window.setTimeout(() => view.add(e), e.t_ms / speed));
        timers.push(
          window.setTimeout(() => {
            stopDemo();
            finished("demo", id); // played to its end: rate it
          }, last / speed + 50),
        );
      });
      stop.addEventListener("click", () => {
        stopDemo();
        demoLines.append(note("Stopped"));
      });
      void loadCatalog();
      void loadSandbox();

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
    private readonly callLine: HTMLElement,
    private readonly label: HTMLInputElement,
    private readonly note: HTMLTextAreaElement,
    private readonly rows: HTMLElement,
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
    this.panel.classList.remove("is-in");
    this.panel.hidden = true;
  }

  /** Show the review of `callId`. `slide`: the call just ended, so the panel slides in. */
  async open(callId: string, name: string, slide: boolean): Promise<void> {
    const mine = ++this.gen;
    this.call = callId;
    this.status.textContent = "";
    this.callLine.textContent = name;
    this.rows.replaceChildren(emptyNote("Loading the rubric"));
    this.panel.hidden = false;
    if (slide && !reducedMotion()) {
      this.panel.classList.remove("is-in");
      void this.panel.offsetWidth; // start from the hidden place
      requestAnimationFrame(() => this.panel.classList.add("is-in"));
      this.panel.scrollIntoView({ block: "nearest", behavior: "smooth" });
    } else {
      this.panel.classList.add("is-in");
    }

    let found: CallRow | undefined;
    let dimensions: Record<string, string> = {};
    try {
      const cat = await catalog();
      found = cat.calls.find((c) => c.id === callId);
      dimensions = cat.dimensions;
    } catch {
      if (mine === this.gen) this.rows.replaceChildren(emptyNote("The agent's dev server is not running"));
      return;
    }
    if (mine !== this.gen) return;
    if (!found) {
      this.rows.replaceChildren(emptyNote("This call is not in the catalog yet"));
      return;
    }
    this.callLine.textContent = callName(found);
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
      await annotate({ ...a, call });
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
    if (state === "passed") icon.innerHTML = CHECK;
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
        l.className = "tool-label";
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

/** The rail's Tools panel for one call (CHAT.md §9): each tool call as a main line (its name
 * and its arguments), and under it the caller's line that led to it, the time to trigger and
 * the time to confirm. Made new for each call, so it clears the panel. */
class ToolLog {
  private lastCaller: CallEvent | undefined;
  /** Rows waiting for their result, by tool name, oldest first. */
  private waiting = new Map<string, ((e: CallEvent) => void)[]>();

  constructor(private readonly box: HTMLElement) {
    box.replaceChildren(emptyNote("No tool calls yet"));
  }

  caller(e: CallEvent): void {
    this.lastCaller = e;
  }

  call(e: CallEvent): void {
    const name = typeof e.name === "string" ? e.name : "tool";
    const row = document.createElement("div");
    row.className = "tool";

    const main = document.createElement("p");
    main.className = "tool-main";
    const n = document.createElement("span");
    n.className = "tool-name";
    n.textContent = name;
    main.append(n);
    const args = argWords(e.arguments);
    if (args) {
      const a = document.createElement("span");
      a.className = "tool-args";
      a.textContent = args;
      main.append(a);
    }

    const facts = document.createElement("ul");
    facts.className = "tool-facts";
    const fact = (label: string, value: string, cls = "") => {
      const li = document.createElement("li");
      if (cls) li.className = cls;
      const l = document.createElement("span");
      l.className = "tool-label";
      l.textContent = `${label} `;
      const v = document.createElement("span");
      v.textContent = value;
      li.append(l, v);
      facts.append(li);
      return v;
    };
    const said = this.lastCaller && typeof this.lastCaller.text === "string" ? this.lastCaller.text : undefined;
    fact("Triggered by", said ? `“${said}”` : "no caller line yet", "tool-said");
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

    row.append(main, facts);
    this.show(row);
  }

  result(e: CallEvent): void {
    const name = typeof e.name === "string" ? e.name : "tool";
    this.waiting.get(name)?.shift()?.(e);
  }

  private show(row: HTMLElement): void {
    const b = this.box;
    // Follow the newest only when the reader is already at the bottom.
    const atEnd = b.scrollHeight - b.scrollTop - b.clientHeight < 8;
    b.querySelector(".empty")?.remove();
    b.append(row);
    if (atEnd) b.scrollTop = b.scrollHeight;
  }
}

/** A tool's arguments as words: the values in order, " · " between. None: an empty string. */
function argWords(args: unknown): string {
  if (!args || typeof args !== "object") return "";
  return Object.values(args as Record<string, unknown>)
    .filter((v) => v !== null && v !== undefined && v !== "")
    .map((v) => (typeof v === "string" ? v : JSON.stringify(v)))
    .join(" · ");
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

function endNote(reason: string): HTMLElement {
  const p = document.createElement("p");
  p.className = "empty";
  p.textContent = reason ? `Call ended: ${reason}` : "Call ended";
  return p;
}

/** A call's name in Demo's picker: the label if it has one, else its time and outcome. */
function callName(c: CallRow): string {
  const when = c.started ? c.started.replace("T", " ").slice(5, 16) : c.id;
  const what = [c.outcome.replace("_", " "), c.scenario].filter(Boolean).join(" · ");
  return `${c.keep ? "★ " : ""}${c.label ?? `${when} · ${what}`}`;
}

/** A rail panel with nothing in it yet. */
function emptyPanel(title: string): string {
  const id = `chat-${title.toLowerCase()}`;
  return (
    `<section class="panel glass panel-${title.toLowerCase()}" aria-labelledby="${id}">` +
    `<h2 id="${id}">${title}</h2><p class="empty">Empty for now</p></section>`
  );
}

const CHEVRON = `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 9l6 6 6-6"/></svg>`;

/** The sandbox patients for the rail: each one's name and birth date, and their visits in a
 * list that one press opens and the next closes. */
function sandboxRows(s: Sandbox): HTMLElement[] {
  return s.patients.map((p, i) => {
    const row = document.createElement("div");
    row.className = "patient";
    const head = document.createElement("div");
    head.className = "patient-head";
    const name = document.createElement("span");
    name.className = "patient-name";
    name.textContent = `${p.given} ${p.family}`;
    const born = document.createElement("span");
    born.className = "patient-born";
    born.textContent = `Born ${longDay(p.birth_date)}`;
    head.append(name, born);
    row.append(head);

    const visits = s.visits.filter((v) => v.patient === p.id).sort((a, b) => a.starts_at.localeCompare(b.starts_at));
    if (!visits.length) {
      const none = document.createElement("p");
      none.className = "visits-none";
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
    toggle.innerHTML = `<span></span>${CHEVRON}`;
    toggle.querySelector("span")!.textContent = visits.length === 1 ? "1 visit" : `${visits.length} visits`;
    const fold = document.createElement("div");
    fold.className = "visits";
    fold.id = listId;
    const inner = document.createElement("div");
    inner.className = "visits-inner";
    const ul = document.createElement("ul");
    for (const v of visits) {
      const li = document.createElement("li");
      li.textContent = `${visitTime(v.starts_at)} · ${v.minutes} min · ${v.clinician} · ${s.place}`;
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
