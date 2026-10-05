# NEXUS architecture

NEXUS is a nested virtual computer that runs entirely in the host browser.

```
Safari / Chrome
  → NEXUS web application (TanStack Start)
    → Web Worker + Rust/WebAssembly
      → RV32IM virtual CPU, RAM, disk, GPU, NIC, input
        → NexusOS (Rust, compiled to RISC-V machine code)
          → Window manager, shell, virtual browser
            → Virtio-style HTTP NIC
              → WASM network bridge (fetch + same-origin proxy)
                → Real Internet (http/https only)
```

This is not an HTML mock of a desktop. The visible screen is a **framebuffer** produced by guest code running on the virtual CPU and presented with WebGPU (WebGL2 / Canvas2D fallback).

## Decision record

### Virtual CPU — custom RV32IM in Rust, compiled to `wasm32-unknown-unknown`

Compared:

| Component | WASM | License | Real OS | Notes |
| --- | --- | --- | --- | --- |
| v86 (JSLinux / copy.sh) | Yes (JS + WASM accel) | BSD | Linux x86 | Mature, large images, not Rust, networking needs a TCP relay |
| CheerpX / WebVM | Yes | Proprietary | Debian | Cannot ship |
| QEMU → WASM | Experimental | GPL | Yes | Huge, slow, poor Safari fit |
| Unicorn | No realistic WASM path | GPL | Yes | JIT/host bindings |
| mini-rv32ima | C, WASM ports exist | zlib-like | Linux RV32 | Tiny and proven; C, not a clean Rust API |
| tinyemu | JS/WASM | MIT | Linux/RISC-V | Bellard; not Rust |
| **NEXUS RV32IM** | **Yes** | MIT (this app) | **NexusOS** | One hart, interpreted, MMIO + SBI |

A hand-written RV32IM interpreter is the only option that is simultaneously: Rust, WASM-friendly, Safari-safe (no JIT/SAB required), auditable, and small enough to ship without a multi-hundred-MB disk image.

It executes **real RISC-V machine code**, including the full NexusOS kernel image produced by `rustc --target riscv32imac-unknown-none-elf` with compressed (`C`) and atomics (`A`) features disabled.

### Virtual OS — NexusOS, not Linux (yet)

A prebuilt RV32 Linux + buildroot image would be more famous, but:

* Typical images are several megabytes plus a rootfs, and boot is tens of billions of instructions on an interpreter (minutes on mobile).
* A framebuffer GUI and a browser inside tiny Linux are not available without shipping X11/Wayland + a real engine (not feasible here).
* We cannot cross-compile the Linux kernel in this environment in a reasonable time.

NexusOS is a real `no_std` Rust kernel: boot, trap/SBI, cooperative apps, VFS, terminal, window manager, browser process. It is **not** Linux and does not pretend to be. Linux bring-up is documented as a later phase (same CPU, different payload).

### Virtual browser — guest chrome + virtio-html device

Servo, Ladybird, Chromium, and WebKit cannot run inside this VM. A full JS engine + CSS layout tree inside interpreted RISC-V would miss interactive frame budgets.

Staged implementation (honest):

1. **Now:** Browser *process* runs on the guest CPU (address bar, tabs, history, keyboard/mouse, navigation). Page bytes arrive through the virtual NIC. A **virtio-html** device (firmware in the WASM module, analogous to a GPU) rasterizes HTML/CSS-lite into the guest framebuffer. No host iframe is ever used for page content.
2. Later: guest-side HTML parser.
3. Later: guest JS (QuickJS compiled to RISC-V) — expected to be very slow.

### Display — WebGPU → WebGL2 → Canvas2D

Guest fills an RGBA8888 framebuffer via GPU SBI commands (`fill`, `text`, `html`, `present`). The worker copies dirty frames to the main thread with transferable buffers. SharedArrayBuffer is used when COOP/COEP allow it; it is **not** required (Safari cross-origin iframes often lack it).

### Persistence — OPFS, then IndexedDB

Virtual disk image (`NEXDISK1`) stored in Origin Private File System, falling back to IndexedDB. Survives reloads on the same origin. No host filesystem access.

### Networking — HTTP(S) only, via a documented bridge

Browsers do not expose raw TCP/UDP to WASM. See [NETWORKING.md](NETWORKING.md).

### Isolation

The VM is untrusted. The only host capabilities are the SBI table (display, input, disk blob, HTTP bridge, clock). No eval, no host process spawn, no arbitrary FS. See [SECURITY.md](SECURITY.md).

## Virtual hardware map

| Region | Contents |
| --- | --- |
| `0x10000000` | UART |
| `0x10001000` | Keyboard ring |
| `0x10002000` | Mouse |
| `0x10003000` | Syscon (halt) |
| `0x10004000` | Millisecond clock |
| `0x40000000` | RGBA framebuffer (also GPU SBI) |
| `0x80000000` | RAM + NexusOS (default 32 MiB) |

SBI (`ecall`, function in `a7`) implements log, 2D fill/text, present/yield, net tx/rx, HTML raster, hit-test, and filesystem.

## Threads

```
Main thread: UI, permission gates, WebGPU present, input capture
Web Worker:  WASM instance, CPU loop, disk/net polling
```

If Workers are unavailable, the CPU loop timeslices on the main thread.

## What this build is not

* Not Linux, not POSIX-complete.
* Not a full web engine (limited CSS, no JavaScript execution in the guest).
* Not raw sockets, WebTransport, or a public TCP proxy.
* Not 4-core / 512 MB / 2 GB — those figures would be a lie on an interpreted WASM hart. Default machine: 1× RV32IM, 32 MiB RAM, 8 MiB disk, HTTP bridge.
