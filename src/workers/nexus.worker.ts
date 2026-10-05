type VmExports = WebAssembly.Exports & {
  memory: WebAssembly.Memory;
  vm_create: (ram: number, width: number, height: number) => number;
  vm_boot: () => number;
  vm_run: (ops: number) => number;
  vm_fb_ptr: () => number;
  vm_fb_width: () => number;
  vm_fb_height: () => number;
  vm_fb_take_dirty: () => number;
  vm_key: (code: number, down: number) => void;
  vm_mouse: (x: number, y: number, buttons: number, wheel: number) => void;
  vm_net_tx_ptr: () => number;
  vm_net_tx_len: () => number;
  vm_net_take_pending: () => number;
  vm_net_rx_ptr: () => number;
  vm_net_rx_cap: () => number;
  vm_net_complete: (status: number, len: number) => void;
  vm_disk_ptr: () => number;
  vm_disk_len: () => number;
  vm_disk_take_dirty: () => number;
  vm_disk_loaded: () => number;
  vm_set_time: (ms: number) => void;
  vm_set_net_enabled: (enabled: number) => void;
  vm_halted: () => number;
  vm_illegal: () => number;
  vm_instr_lo: () => number;
  vm_instr_hi: () => number;
};

type MainMessage =
  | { type: "start"; network: boolean }
  | { type: "key"; code: number; down: boolean }
  | { type: "mouse"; x: number; y: number; buttons: number; wheel: number }
  | { type: "network"; enabled: boolean }
  | { type: "stop" };

const DB_NAME = "nexus-origin-disk";
const STORE_NAME = "images";
const IMAGE_KEY = "primary";
const WASM_URL = "/nexus-vm.wasm";
let api: VmExports | null = null;
let running = false;
let networkEnabled = true;
let bootAt = 0;
let mouseX = 0;
let mouseY = 0;
let mouseButtons = 0;
let mouseWheel = 0;
let mouseReleaseAt = 0;
let mouseReleasePending = false;

function diskDatabase(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, 1);
    request.onupgradeneeded = () => request.result.createObjectStore(STORE_NAME);
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

async function readDisk(): Promise<Uint8Array | null> {
  try {
    const db = await diskDatabase();
    return await new Promise((resolve, reject) => {
      const request = db.transaction(STORE_NAME, "readonly").objectStore(STORE_NAME).get(IMAGE_KEY);
      request.onsuccess = () => resolve(request.result instanceof ArrayBuffer ? new Uint8Array(request.result) : null);
      request.onerror = () => reject(request.error);
    });
  } catch {
    return null;
  }
}

async function writeDisk(data: ArrayBuffer): Promise<void> {
  try {
    const db = await diskDatabase();
    await new Promise<void>((resolve, reject) => {
      const request = db.transaction(STORE_NAME, "readwrite").objectStore(STORE_NAME).put(data, IMAGE_KEY);
      request.onsuccess = () => resolve();
      request.onerror = () => reject(request.error);
    });
  } catch {
    postMessage({ type: "status", status: "Disk persistence unavailable; this session is temporary." });
  }
}

function memoryBytes(pointer: number, length: number): Uint8Array {
  if (!api) throw new Error("VM memory is unavailable");
  return new Uint8Array(api.memory.buffer, pointer, length);
}

async function fetchGuestUrl(): Promise<void> {
  if (!api) return;
  const length = Math.min(api.vm_net_tx_len(), 2048);
  const url = new TextDecoder().decode(memoryBytes(api.vm_net_tx_ptr(), length));
  try {
    const response = await fetch("/api/net/proxy", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ url }),
      signal: AbortSignal.timeout(13000),
    });
    if (!response.ok) {
      const detail = (await response.text()).slice(0, 160);
      throw new Error(`HTTP bridge rejected the request (${response.status}): ${detail}`);
    }
    const bytes = new Uint8Array(await response.arrayBuffer());
    const capacity = api.vm_net_rx_cap();
    if (bytes.length > capacity) throw new Error("Page is larger than the guest network buffer");
    memoryBytes(api.vm_net_rx_ptr(), bytes.length).set(bytes);
    api.vm_net_complete(0, bytes.length);
    postMessage({ type: "status", status: `Loaded ${new URL(url).hostname}` });
  } catch (error) {
    api.vm_net_complete(-1, 0);
    postMessage({ type: "status", status: error instanceof Error ? error.message : "Network request failed" });
  }
}

function present(): void {
  if (!api || !api.vm_fb_take_dirty()) return;
  const width = api.vm_fb_width();
  const height = api.vm_fb_height();
  const start = api.vm_fb_ptr();
  const frame = memoryBytes(start, width * height * 4).slice();
  postMessage({ type: "frame", width, height, pixels: frame.buffer }, { transfer: [frame.buffer] });
}

function flushDisk(): void {
  if (!api || !api.vm_disk_take_dirty()) return;
  const image = memoryBytes(api.vm_disk_ptr(), api.vm_disk_len()).slice();
  void writeDisk(image.buffer);
}

async function tick(): Promise<void> {
  if (!running || !api) return;
  api.vm_set_time((performance.now() - bootAt) >>> 0);
  if (mouseReleasePending && performance.now() >= mouseReleaseAt) {
    mouseButtons = 0;
    mouseReleasePending = false;
  }
  api.vm_mouse(mouseX, mouseY, mouseButtons, mouseWheel);
  mouseWheel = 0;
  api.vm_run(60000);
  present();
  const high = api.vm_instr_hi();
  const low = api.vm_instr_lo();
  const instructions = high * 4294967296 + low;
  postMessage({ type: "metrics", instructions, halted: api.vm_halted() !== 0, error: api.vm_illegal() !== 0 });
  flushDisk();
  if (api.vm_net_take_pending()) {
    await fetchGuestUrl();
  }
  if (running) setTimeout(tick, 16);
}

async function start(network: boolean): Promise<void> {
  if (running) return;
  networkEnabled = network;
  postMessage({ type: "status", status: "Loading RISC-V virtual machine…" });
  try {
    // Avoid reusing an older cached guest image after a deployment updates the kernel.
    const response = await fetch(`${WASM_URL}?v=${Date.now()}`, { cache: "no-store" });
    if (!response.ok) throw new Error(`Virtual machine image could not be loaded (${response.status})`);
    const result = await WebAssembly.instantiate(await response.arrayBuffer(), {});
    api = result.instance.exports as VmExports;
    if (!(api.memory instanceof WebAssembly.Memory)) throw new Error("WASM memory export is missing");
    api.vm_create(32, 960, 600);
    api.vm_set_net_enabled(networkEnabled ? 1 : 0);
    const saved = await readDisk();
    if (saved && saved.length === api.vm_disk_len() && new TextDecoder().decode(saved.subarray(0, 8)) === "NEXDISK1") {
      memoryBytes(api.vm_disk_ptr(), saved.length).set(saved);
      api.vm_disk_loaded();
      postMessage({ type: "status", status: "Restored your virtual disk." });
    }
    if (api.vm_boot() !== 0) throw new Error("NexusOS failed to boot");
    bootAt = performance.now();
    running = true;
    postMessage({ type: "status", status: "NexusOS 0.1 · RV32IM · 32 MiB RAM" });
    await tick();
  } catch (error) {
    running = false;
    postMessage({ type: "error", message: error instanceof Error ? error.message : "Could not start NEXUS" });
  }
}

self.onmessage = (event: MessageEvent<MainMessage>) => {
  const message = event.data;
  if (message.type === "start") void start(message.network);
  if (message.type === "stop") {
    running = false;
    flushDisk();
  }
  if (message.type === "network") {
    networkEnabled = message.enabled;
    api?.vm_set_net_enabled(networkEnabled ? 1 : 0);
    postMessage({ type: "status", status: networkEnabled ? "Internet bridge enabled" : "Internet bridge disabled" });
  }
  if (message.type === "key") api?.vm_key(message.code, message.down ? 1 : 0);
  if (message.type === "mouse") {
    mouseX = message.x;
    mouseY = message.y;
    mouseWheel = message.wheel;
    if (message.buttons > 0) {
      mouseButtons = message.buttons;
      mouseReleaseAt = performance.now() + 50;
      mouseReleasePending = false;
    } else if (mouseButtons > 0 && performance.now() < mouseReleaseAt) {
      mouseReleasePending = true;
    } else {
      mouseButtons = 0;
      mouseReleasePending = false;
    }
  }
};
