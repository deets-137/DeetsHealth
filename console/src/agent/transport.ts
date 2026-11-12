// The caller's seat for a typed test call (CHAT.md §5): RTVI as JSON text over one WebSocket,
// no audio and no devices.
//
// Why our own: Pipecat's browser WebSocket transport pulls in daily-js, which loads Daily's
// call-machine bundle from Daily's CDN, and a page that shows protected data runs no remote
// code (SECURITY.md §4). `@pipecat-ai/client-js` itself has no such dependency, so we keep its
// client (sendText, the RTVI events) and give it this transport. The server's half is
// agent/deets_agent/rtvi_json.py. Voice in the browser (SmallWebRTC) needs the same check.
import {
  RTVIMessage,
  Transport,
  type PipecatClientOptions,
  type Tracks,
  type TransportConnectionParams,
  type TransportState,
} from "@pipecat-ai/client-js";

const NONE = {} as Record<string, never>;

export class TextSeat extends Transport {
  private ws: WebSocket | undefined;
  private readonly url: string;

  /** `path` is on this page's own origin; Vite's proxy takes it to the agent (vite.config.ts). */
  constructor(path = "/agent/call") {
    super();
    const scheme = location.protocol === "https:" ? "wss:" : "ws:";
    this.url = `${scheme}//${location.host}${path}`;
    this._state = "disconnected";
  }

  initialize(options: PipecatClientOptions, messageHandler: (ev: RTVIMessage) => void): void {
    this._options = options;
    this._callbacks = options.callbacks ?? {};
    this._onMessage = messageHandler;
    this.state = "disconnected";
  }

  async initDevices(): Promise<void> {
    this.state = "initialized"; // a text seat has no devices
  }

  _validateConnectionParams(connectParams?: unknown): unknown {
    return connectParams; // the seat's address is fixed; nothing to check
  }

  async _connect(_connectParams?: TransportConnectionParams): Promise<void> {
    this.state = "connecting";
    const ws = new WebSocket(this.url);
    this.ws = ws;
    await new Promise<void>((ok, fail) => {
      ws.addEventListener("open", () => ok(), { once: true });
      ws.addEventListener("error", () => fail(new Error("the agent did not answer")), { once: true });
      ws.addEventListener("close", (e) => fail(new Error(e.reason || "the agent refused the call")), { once: true });
    });
    ws.addEventListener("message", (e) => {
      if (typeof e.data !== "string") return;
      try {
        const m = JSON.parse(e.data);
        if (m && m.label === "rtvi-ai") this._onMessage(m as RTVIMessage);
      } catch {
        /* a line that is not JSON is dropped; it never carries anything we show */
      }
    });
    ws.addEventListener("close", () => {
      if (this.state === "disconnecting" || this.state === "disconnected") return;
      this.state = "disconnected";
      this._callbacks.onDisconnected?.();
    });
    this.state = "connected";
    this._callbacks.onConnected?.();
  }

  async _disconnect(): Promise<void> {
    this.state = "disconnecting";
    this.ws?.close(1000);
    this.ws = undefined;
    this.state = "disconnected";
    this._callbacks.onDisconnected?.();
  }

  sendReadyMessage(): void {
    this.state = "ready";
    this.sendMessage(RTVIMessage.clientReady());
  }

  sendMessage(message: RTVIMessage): void {
    if (this.ws?.readyState === WebSocket.OPEN) this.ws.send(JSON.stringify(message));
  }

  get state(): TransportState {
    return this._state;
  }
  set state(state: TransportState) {
    if (this._state === state) return;
    this._state = state;
    this._callbacks?.onTransportStateChanged?.(state);
  }

  // ---- no devices: a typed seat has no mic, camera or speaker
  async getAllMics(): Promise<MediaDeviceInfo[]> {
    return [];
  }
  async getAllCams(): Promise<MediaDeviceInfo[]> {
    return [];
  }
  async getAllSpeakers(): Promise<MediaDeviceInfo[]> {
    return [];
  }
  updateMic(): void {}
  updateCam(): void {}
  updateSpeaker(): void {}
  get selectedMic() {
    return NONE;
  }
  get selectedCam() {
    return NONE;
  }
  get selectedSpeaker() {
    return NONE;
  }
  enableMic(): void {}
  enableCam(): void {}
  enableScreenShare(): void {}
  get isCamEnabled(): boolean {
    return false;
  }
  get isMicEnabled(): boolean {
    return false;
  }
  get isSharingScreen(): boolean {
    return false;
  }
  tracks(): Tracks {
    return { local: {} };
  }
}
