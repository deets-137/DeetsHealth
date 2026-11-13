// The caller's seat for a typed test call (CHAT.md §5): RTVI as JSON text over one WebSocket,
// no audio and no devices.
//
// Why our own: Pipecat's browser WebSocket transport pulls in daily-js, which loads Daily's
// call-machine bundle from Daily's CDN, and a page that shows protected data runs no remote
// code (SECURITY.md §4). `@pipecat-ai/client-js` itself has no such dependency, so we keep its
// client (sendText, the RTVI events) and give it this transport. The server's half is
// agent/deets_agent/rtvi_json.py. Voice in the browser (VoiceSeat below) passed the same check:
// it uses the browser's own WebRTC and no package from Pipecat or Daily.
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

  /** `path` is on this page's own origin; Vite's proxy takes it to the agent (vite.config.ts).
   * Its query may name the EHR the call reads (`?ehr=epic`, CHAT.md §13). */
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

/** How often the voice seat tells the server it is still here. Pipecat's SmallWebRTC server
 * counts a seat gone after 3 s with no message (connection.py › is_connected). */
const PING_MS = 1000;
/** The longest wait for the browser to gather its ICE candidates before it sends the offer. On
 * one machine the host candidates come at once. */
const GATHER_MS = 2000;

/** The caller's seat for a voice test call (CHAT.md §17): the laptop microphone and speaker over
 * WebRTC to Pipecat's SmallWebRTC server, and RTVI as JSON on the data channel, so typed lines
 * still work.
 *
 * Why our own: Pipecat's browser package for SmallWebRTC depends on daily-js, which loads code
 * from Daily's CDN (the same rule as TextSeat above). This seat uses the browser's own
 * RTCPeerConnection and nothing else. The server's half is the dev server's `POST /agent/voice`
 * (agent/deets_agent/dev_server.py), which takes the offer and returns the answer. */
export class VoiceSeat extends Transport {
  private pc: RTCPeerConnection | undefined;
  private channel: RTCDataChannel | undefined;
  private mic: MediaStreamTrack | undefined;
  private speaker: HTMLAudioElement | undefined;
  private ping = 0;
  private micOn = true;

  /** `offer` sends the browser's offer to the dev server and returns its answer (api.ts). */
  constructor(private readonly offer: (o: RTCSessionDescriptionInit) => Promise<RTCSessionDescriptionInit>) {
    super();
    this._state = "disconnected";
  }

  initialize(options: PipecatClientOptions, messageHandler: (ev: RTVIMessage) => void): void {
    this._options = options;
    this._callbacks = options.callbacks ?? {};
    this._onMessage = messageHandler;
    this.state = "disconnected";
  }

  async initDevices(): Promise<void> {
    this.state = "initializing";
    try {
      const stream = await navigator.mediaDevices.getUserMedia({
        audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true },
        video: false,
      });
      this.mic = stream.getAudioTracks()[0];
    } catch {
      this.state = "disconnected";
      throw new Error("the browser did not allow the microphone");
    }
    this.state = "initialized";
  }

  _validateConnectionParams(connectParams?: unknown): unknown {
    return connectParams; // the seat's address is fixed; nothing to check
  }

  async _connect(_connectParams?: TransportConnectionParams): Promise<void> {
    if (!this.mic) await this.initDevices();
    this.state = "connecting";
    // On this machine both ends are local: host candidates only, no STUN or TURN server.
    const pc = new RTCPeerConnection();
    this.pc = pc;
    pc.addTransceiver(this.mic!, { direction: "sendrecv" });
    pc.addEventListener("track", (e) => {
      if (e.track.kind !== "audio") return;
      const audio = new Audio();
      audio.autoplay = true;
      audio.srcObject = new MediaStream([e.track]);
      this.speaker = audio;
      void audio.play().catch(() => {});
    });
    const channel = pc.createDataChannel("rtvi", { ordered: true });
    this.channel = channel;
    channel.addEventListener("message", (e) => {
      if (typeof e.data !== "string") return;
      try {
        const m = JSON.parse(e.data);
        if (m && m.label === "rtvi-ai") this._onMessage(m as RTVIMessage);
        else if (m && m.type === "signalling" && m.message?.type === "peerLeft") void this.ended();
      } catch {
        /* a line that is not JSON is dropped; it never carries anything we show */
      }
    });
    pc.addEventListener("connectionstatechange", () => {
      if (pc.connectionState === "failed" || pc.connectionState === "closed") void this.ended();
    });

    await pc.setLocalDescription(await pc.createOffer());
    await gathered(pc);
    try {
      const answer = await this.offer(pc.localDescription!);
      await pc.setRemoteDescription(answer);
      await new Promise<void>((ok, fail) => {
        if (channel.readyState === "open") return ok();
        const timer = window.setTimeout(() => fail(new Error("the agent did not answer")), 15_000);
        channel.addEventListener("open", () => (window.clearTimeout(timer), ok()), { once: true });
      });
    } catch (e) {
      this.close();
      this.state = "disconnected";
      throw e;
    }
    this.ping = window.setInterval(() => {
      if (channel.readyState === "open") channel.send(`ping: ${Date.now()}`);
    }, PING_MS);
    this.state = "connected";
    this._callbacks.onConnected?.();
  }

  /** The other end left, or the connection failed. */
  private async ended(): Promise<void> {
    if (this.state === "disconnecting" || this.state === "disconnected") return;
    this.close();
    this.state = "disconnected";
    this._callbacks.onDisconnected?.();
  }

  private close(): void {
    window.clearInterval(this.ping);
    this.channel?.close();
    this.pc?.close();
    this.mic?.stop();
    if (this.speaker) this.speaker.srcObject = null;
    this.channel = undefined;
    this.pc = undefined;
    this.mic = undefined;
    this.speaker = undefined;
  }

  async _disconnect(): Promise<void> {
    this.state = "disconnecting";
    this.close();
    this.state = "disconnected";
    this._callbacks.onDisconnected?.();
  }

  sendReadyMessage(): void {
    this.state = "ready";
    this.sendMessage(RTVIMessage.clientReady());
  }

  sendMessage(message: RTVIMessage): void {
    if (this.channel?.readyState === "open") this.channel.send(JSON.stringify(message));
  }

  get state(): TransportState {
    return this._state;
  }
  set state(state: TransportState) {
    if (this._state === state) return;
    this._state = state;
    this._callbacks?.onTransportStateChanged?.(state);
  }

  // ---- devices: the default microphone and speaker; no camera, no screen
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
  enableMic(enable: boolean): void {
    this.micOn = enable;
    if (this.mic) this.mic.enabled = enable;
  }
  enableCam(): void {}
  enableScreenShare(): void {}
  get isCamEnabled(): boolean {
    return false;
  }
  get isMicEnabled(): boolean {
    return this.micOn;
  }
  get isSharingScreen(): boolean {
    return false;
  }
  tracks(): Tracks {
    return { local: this.mic ? { audio: this.mic } : {} };
  }
}

/** Resolves when the browser has gathered its ICE candidates, or after GATHER_MS. */
function gathered(pc: RTCPeerConnection): Promise<void> {
  if (pc.iceGatheringState === "complete") return Promise.resolve();
  return new Promise((ok) => {
    const timer = window.setTimeout(ok, GATHER_MS);
    pc.addEventListener("icegatheringstatechange", () => {
      if (pc.iceGatheringState === "complete") {
        window.clearTimeout(timer);
        ok();
      }
    });
  });
}
