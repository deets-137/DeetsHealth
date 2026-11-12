// Chat: a large stage and a rail of four panels, laid out as DeetsMotion's Visualizer.
// A skeleton: the panels are empty. Their names are a draft for the owner to edit
// (CONSOLE.md §3).
import { el, type Page } from "./page";

export function chatPage(): Page {
  return {
    id: "chat",
    title: "Chat",
    build() {
      return el(`
        <section class="page">
          <div class="split">
            <section class="stage glass" aria-labelledby="chat-title">
              <header class="stage-head">
                <h1 id="chat-title">Chat</h1>
              </header>
              <div class="stage-body">
                <p class="empty">Empty for now</p>
              </div>
            </section>

            <aside class="rail" aria-label="Chat details">
              <section class="panel glass" aria-labelledby="chat-timeline">
                <h2 id="chat-timeline">Timeline</h2>
                <p class="empty">Empty for now</p>
              </section>
              <section class="panel glass" aria-labelledby="chat-caller">
                <h2 id="chat-caller">Caller</h2>
                <p class="empty">Empty for now</p>
              </section>
              <section class="panel glass" aria-labelledby="chat-step">
                <h2 id="chat-step">Step</h2>
                <p class="empty">Empty for now</p>
              </section>
              <section class="panel glass" aria-labelledby="chat-tools">
                <h2 id="chat-tools">Tools</h2>
                <p class="empty">Empty for now</p>
              </section>
            </aside>
          </div>
        </section>`);
    },
  };
}
