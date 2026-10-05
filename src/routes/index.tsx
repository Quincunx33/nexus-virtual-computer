import { createFileRoute } from "@tanstack/react-router";
import { Activity, ArrowUpRight, Cpu, HardDrive, Monitor, Power, ShieldCheck, Wifi, WifiOff, Zap } from "lucide-react";
import { useEffect, useRef, useState } from "react";

export const Route = createFileRoute("/")({ component: Home });

type WorkerMessage =
  | { type: "frame"; width: number; height: number; pixels: ArrayBuffer }
  | { type: "status"; status: string }
  | { type: "metrics"; instructions: number; halted: boolean; error: boolean }
  | { type: "error"; message: string };

type MachineState = "off" | "booting" | "running" | "error";

function keyCode(event: KeyboardEvent): number | null {
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "a") return 0x84;
  if (event.code === "Enter") return 13;
  if (event.code === "Backspace") return 8;
  if (event.code === "Escape") return 27;
  if (event.code === "Tab") return 9;
  if (event.code === "ArrowLeft") return 0x80;
  if (event.code === "ArrowRight") return 0x81;
  if (event.code === "ArrowUp") return 0x82;
  if (event.code === "ArrowDown") return 0x83;
  if (event.code === "Home") return 0x85;
  if (event.code === "End") return 0x86;
  if (event.code === "Delete") return 0x87;
  if (event.code === "PageUp") return 0x88;
  if (event.code === "PageDown") return 0x89;
  if (event.key.length === 1 && event.key.charCodeAt(0) < 128) return event.key.charCodeAt(0);
  return null;
}

function formatInstructions(value: number): string {
  if (!Number.isFinite(value)) return "0";
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`;
  return String(Math.floor(value));
}

function Home() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const workerRef = useRef<Worker | null>(null);
  const runningRef = useRef(false);
  const [state, setState] = useState<MachineState>("off");
  const [status, setStatus] = useState("Ready when you are");
  const [network, setNetwork] = useState(true);
  const [instructions, setInstructions] = useState(0);
  const [machineError, setMachineError] = useState("");
  const [uptime, setUptime] = useState(0);
  const [touchText, setTouchText] = useState("");
  const touchTextRef = useRef("");
  const composingRef = useRef(false);

  useEffect(() => {
    const worker = new Worker(new URL("../workers/nexus.worker.ts", import.meta.url), { type: "module" });
    workerRef.current = worker;
    worker.onmessage = (event: MessageEvent<WorkerMessage>) => {
      const message = event.data;
      if (message.type === "frame") {
        const canvas = canvasRef.current;
        if (!canvas) return;
        const ctx = canvas.getContext("2d", { alpha: false });
        if (!ctx) return;
        if (canvas.width !== message.width || canvas.height !== message.height) {
          canvas.width = message.width;
          canvas.height = message.height;
        }
        ctx.putImageData(new ImageData(new Uint8ClampedArray(message.pixels), message.width, message.height), 0, 0);
      } else if (message.type === "status") {
        setStatus(message.status);
      } else if (message.type === "metrics") {
        setInstructions(message.instructions);
        if (message.error) {
          setState("error");
          setMachineError("Guest CPU stopped on an illegal instruction.");
        } else if (message.halted) {
          setStatus("Guest halted");
        }
      } else {
        setState("error");
        setMachineError(message.message);
        setStatus("Could not start the virtual machine");
      }
    };
    worker.onerror = (event) => {
      setState("error");
      setMachineError(event.message || "Virtual machine worker failed.");
    };
    const sendKey = (event: KeyboardEvent, down: boolean) => {
      if (!runningRef.current || document.activeElement !== canvasRef.current) return;
      const code = keyCode(event);
      if (code === null) return;
      event.preventDefault();
      worker.postMessage({ type: "key", code, down });
    };
    const onDown = (event: KeyboardEvent) => sendKey(event, true);
    const onUp = (event: KeyboardEvent) => sendKey(event, false);
    window.addEventListener("keydown", onDown);
    window.addEventListener("keyup", onUp);
    return () => {
      runningRef.current = false;
      worker.postMessage({ type: "stop" });
      worker.terminate();
      workerRef.current = null;
      window.removeEventListener("keydown", onDown);
      window.removeEventListener("keyup", onUp);
    };
  }, []);

  useEffect(() => {
    if (state !== "running") return;
    const start = Date.now();
    const timer = window.setInterval(() => setUptime(Math.floor((Date.now() - start) / 1000)), 1000);
    return () => window.clearInterval(timer);
  }, [state]);

  function powerOn() {
    if (!workerRef.current) return;
    runningRef.current = true;
    setMachineError("");
    setState("booting");
    setStatus("Starting NexusOS…");
    setInstructions(0);
    setUptime(0);
    workerRef.current.postMessage({ type: "start", network });
    window.setTimeout(() => {
      if (runningRef.current) setState((current) => current === "booting" ? "running" : current);
    }, 300);
  }

  function powerOff() {
    runningRef.current = false;
    workerRef.current?.postMessage({ type: "stop" });
    setState("off");
    setStatus("Powered off · virtual disk saved locally");
    setInstructions(0);
    setUptime(0);
  }

  function toggleNetwork() {
    const next = !network;
    setNetwork(next);
    workerRef.current?.postMessage({ type: "network", enabled: next });
  }

  function pointerCoordinates(event: { currentTarget: HTMLCanvasElement; clientX: number; clientY: number }) {
    const rect = event.currentTarget.getBoundingClientRect();
    const sourceWidth = event.currentTarget.width || 960;
    const sourceHeight = event.currentTarget.height || 600;
    const scale = Math.min(rect.width / sourceWidth, rect.height / sourceHeight);
    const renderedWidth = sourceWidth * scale;
    const renderedHeight = sourceHeight * scale;
    const insetX = (rect.width - renderedWidth) / 2;
    const insetY = (rect.height - renderedHeight) / 2;
    return {
      x: Math.max(0, Math.min(sourceWidth - 1, Math.round(((event.clientX - rect.left - insetX) / renderedWidth) * sourceWidth))),
      y: Math.max(0, Math.min(sourceHeight - 1, Math.round(((event.clientY - rect.top - insetY) / renderedHeight) * sourceHeight))),
    };
  }

  function sendMouse(event: { currentTarget: HTMLCanvasElement; clientX: number; clientY: number }, buttons: number, wheel = 0) {
    const point = pointerCoordinates(event);
    workerRef.current?.postMessage({ type: "mouse", ...point, buttons, wheel });
  }

  function sendGuestKey(code: number) {
    if (!runningRef.current || !workerRef.current) return;
    workerRef.current.postMessage({ type: "key", code, down: true });
    workerRef.current.postMessage({ type: "key", code, down: false });
  }

  function sendTouchKey(code: number) {
    if (!runningRef.current || !workerRef.current) return;
    // The guest shell consumes key-downs only; a paired key-up doubled the queue
    // load for pasted mobile commands without adding any guest behavior.
    workerRef.current.postMessage({ type: "key", code, down: true });
  }

  function updateTouchText(next: string) {
    setTouchText(next);
    if (composingRef.current) return;
    const previous = touchTextRef.current;
    if (next.startsWith(previous)) {
      for (const char of next.slice(previous.length)) {
        const code = char.charCodeAt(0);
        if (code >= 0x20 && code <= 0x7e) sendTouchKey(code);
      }
    } else if (previous.startsWith(next)) {
      for (let i = next.length; i < previous.length; i += 1) sendTouchKey(8);
    } else {
      let common = 0;
      while (common < previous.length && common < next.length && previous[common] === next[common]) common += 1;
      for (let i = common; i < previous.length; i += 1) sendTouchKey(8);
      for (const char of next.slice(common)) {
        const code = char.charCodeAt(0);
        if (code >= 0x20 && code <= 0x7e) sendTouchKey(code);
      }
    }
    touchTextRef.current = next;
  }

  function submitTouchText() {
    sendTouchKey(13);
    touchTextRef.current = "";
    setTouchText("");
  }

  const stateLabel = state === "running" ? "SYSTEM ONLINE" : state === "booting" ? "BOOT SEQUENCE" : state === "error" ? "SYSTEM FAULT" : "STANDBY";
  const minutes = Math.floor(uptime / 60).toString().padStart(2, "0");
  const seconds = (uptime % 60).toString().padStart(2, "0");

  return (
    <main className="app-shell">
      <header className="topbar">
        <a className="brand" href="#home" aria-label="NEXUS home">
          <span className="brand-mark"><span /></span>
          <span className="brand-name">NEXUS<span className="brand-slash">/</span></span>
          <span className="brand-edition">VIRTUAL SYSTEM</span>
        </a>
        <div className="topbar-right">
          <span className="secure-label"><ShieldCheck size={14} /> SANDBOXED</span>
          <span className="top-divider" />
          <span className="build-label">BUILD 0.1.0 <span className="build-dot" /></span>
        </div>
      </header>

      <div className="workspace">
        <section className="hero-row">
          <div>
            <div className="eyebrow"><span className="eyebrow-line" /> PERSONAL COMPUTING, REIMAGINED</div>
            <h1>Your own little <span>universe.</span></h1>
            <p className="hero-copy">A complete virtual computer. Powered by real RISC-V code, running right here in your browser.</p>
          </div>
          <button className={`power-button ${state === "running" || state === "booting" ? "is-live" : ""}`} onClick={state === "running" || state === "booting" ? powerOff : powerOn}>
            {state === "running" || state === "booting" ? <Power size={16} /> : <Zap size={16} />}
            {state === "running" || state === "booting" ? "Power off" : "Power on"}
          </button>
        </section>

        <section className="machine-layout" aria-label="Virtual computer">
          <div className="machine-main">
            <div className="machine-bar">
              <div className="machine-name"><Monitor size={15} /><span>NEXUS WORKSTATION</span><span className="machine-tag">N-01</span></div>
              <div className="machine-state"><span className={`state-light ${state}`} />{stateLabel}</div>
            </div>
            <div className="screen-outer">
              <div className="screen-bezel">
                <canvas
                  ref={canvasRef}
                  className={`vm-screen ${state === "off" || state === "error" ? "vm-screen-off" : ""}`}
                  width={960}
                  height={600}
                  aria-label="NexusOS virtual computer display. Click the display, then type to use the guest desktop."
                  onPointerDown={(event) => {
                    event.currentTarget.focus({ preventScroll: true });
                    event.currentTarget.setPointerCapture(event.pointerId);
                    sendMouse(event, 1);
                  }}
                  onPointerMove={(event) => sendMouse(event, event.buttons ? 1 : 0)}
                  onPointerUp={(event) => sendMouse(event, 0)}
                  onPointerCancel={(event) => sendMouse(event, 0)}
                  onWheel={(event) => sendMouse(event, 0, Math.sign(event.deltaY))}
                  tabIndex={0}
                />
                {(state === "off" || state === "error") && (
                  <div className="screen-overlay">
                    <div className="orbital-mark"><span className="orbital-core" /><span className="orbit orbit-a" /><span className="orbit orbit-b" /><span className="orbit orbit-c" /></div>
                    <p className="screen-kicker">{state === "error" ? "SYSTEM INTERRUPTED" : "READY FOR LIFTOFF"}</p>
                    <h2>{state === "error" ? "We hit a snag." : "A computer, within a computer."}</h2>
                    <p className="screen-detail">{state === "error" ? machineError : "Boot a real RISC-V guest OS. Your files stay in this browser."}</p>
                    <button className="screen-start" onClick={state === "error" ? powerOn : powerOn}><Power size={14} /> {state === "error" ? "Try again" : "Start NEXUS"}</button>
                  </div>
                )}
                {state === "booting" && <div className="boot-indicator"><span className="loader-ring" />{status}</div>}
              </div>
              <div className="bezel-brand"><span className="brand-mark small"><span /></span> NEXUS <span>ENGINEERED FOR CURIOSITY</span></div>
            </div>
            <div className="machine-controls">
              <div className="status-message"><span className={`state-light ${state}`} /> <span>{status}</span></div>
              <div className="quick-controls">
                <button className={`network-toggle ${network ? "connected" : ""}`} onClick={toggleNetwork} title={network ? "Disable guest Internet access" : "Enable guest Internet access"} aria-label={network ? "Disable internet bridge" : "Enable internet bridge"}>
                  {network ? <Wifi size={15} /> : <WifiOff size={15} />}<span>NET {network ? "ON" : "OFF"}</span>
                </button>
                <span className="control-divider" />
                <span className="uptime"><Activity size={14} /> {minutes}:{seconds}</span>
              </div>
            </div>
            <div className="touch-keyboard" aria-label="Touchscreen guest keyboard">
              <div className="touch-shortcuts" aria-label="Virtual keyboard shortcuts">
                <button type="button" onClick={() => sendGuestKey(0x84)} disabled={state !== "running"} aria-label="Select the guest browser address or move to the start of the terminal line" title="Select URL / line">URL</button>
                <button type="button" onClick={() => sendGuestKey(9)} disabled={state !== "running"} aria-label="Tab" title="Tab">TAB</button>
                <button type="button" onClick={() => sendGuestKey(0x82)} disabled={state !== "running"} aria-label="Previous command or page up" title="Previous command / page up">↑</button>
                <button type="button" onClick={() => sendGuestKey(0x83)} disabled={state !== "running"} aria-label="Next command or page down" title="Next command / page down">↓</button>
                <button type="button" onClick={() => sendGuestKey(27)} disabled={state !== "running"} aria-label="Escape" title="Escape">ESC</button>
              </div>
              <div className="touch-input-row">
                <input
                  aria-label="Type into the virtual computer"
                  autoCapitalize="off"
                  autoComplete="off"
                  autoCorrect="off"
                  className="touch-keyboard-input"
                  disabled={state !== "running"}
                  enterKeyHint="send"
                  inputMode="text"
                  maxLength={250}
                  onCompositionEnd={(event) => {
                    composingRef.current = false;
                    updateTouchText(event.currentTarget.value);
                  }}
                  onCompositionStart={() => { composingRef.current = true; }}
                  onChange={(event) => updateTouchText(event.currentTarget.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter" && !event.nativeEvent.isComposing) {
                      event.preventDefault();
                      submitTouchText();
                    }
                  }}
                  placeholder={state === "running" ? "Type a command or URL" : "Power on to type"}
                  spellCheck={false}
                  value={touchText}
                />
                <button type="button" onClick={submitTouchText} disabled={state !== "running"} aria-label="Send input to the virtual computer">SEND</button>
              </div>
            </div>
          </div>

          <aside className="inspector">
            <div className="inspector-heading"><span>HARDWARE</span><span className="inspector-live"><i /> LIVE</span></div>
            <div className="spec-card">
              <div className="spec-icon"><Cpu size={17} /></div>
              <div className="spec-copy"><span className="spec-label">PROCESSOR</span><strong>RV32IM</strong><small>Single hart · 32-bit</small></div>
              <span className="spec-status">READY</span>
            </div>
            <div className="spec-card">
              <div className="spec-icon"><HardDrive size={17} /></div>
              <div className="spec-copy"><span className="spec-label">MEMORY</span><strong>32 <em>MiB</em></strong><small>Guest RAM</small></div>
              <span className="spec-status">32 MB</span>
            </div>
            <div className="spec-card">
              <div className="spec-icon"><Monitor size={17} /></div>
              <div className="spec-copy"><span className="spec-label">DISPLAY</span><strong>960 × 600</strong><small>Pixel framebuffer</small></div>
              <span className="spec-status">60 HZ</span>
            </div>

            <div className="metrics-panel">
              <div className="metrics-title"><span>PERFORMANCE</span><span className="metrics-sub">SESSION</span></div>
              <div className="metric-row"><span>Instructions</span><strong>{formatInstructions(instructions)}</strong></div>
              <div className="meter-track"><span style={{ width: `${state === "running" ? Math.min(100, 26 + (instructions % 40)) : 0}%` }} /></div>
              <div className="metric-note"><span className={state === "running" ? "metric-live" : ""} /> {state === "running" ? "Guest CPU active" : "Awaiting startup"}</div>
            </div>

            <div className="feature-note"><div className="feature-icon"><ArrowUpRight size={15} /></div><div><strong>Built for the open web</strong><p>Browse static pages through an opt-in, safety-limited HTTP bridge.</p></div></div>
            <div className="keyboard-hint"><span>INPUT</span><p>Click inside the screen to use the guest desktop. <kbd>Esc</kbd> returns to the desktop.</p></div>
          </aside>
        </section>

        <footer className="page-footer"><span>AN EXPERIMENT IN PERSONAL COMPUTING</span><span>NEXUS OS <b>·</b> RV32IM <b>·</b> BUILT TO EXPLORE</span></footer>
      </div>
    </main>
  );
}
