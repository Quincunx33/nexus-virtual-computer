# Performance

## Targets

* Present 60 fps when the framebuffer is dirty; skip clean frames.
* Keep the UI thread outside the CPU loop (Web Worker).
* Avoid JS ↔ WASM copies except: dirty framebuffer, net payloads, disk snapshots.

## Reality of an interpreter

A single interpreted RV32 hart in WASM is roughly mid-single-digit to low-tens of MIPS depending on the host. That is enough for:

* NexusOS boot (milliseconds of guest work + staged yields so the log is visible)
* Window manager, terminal, browser chrome
* HTML raster in firmware (not in interpreted guest loops over pixels)

It is **not** enough for Linux + X11 + a real engine at interactive rates, which is why that stack is not shipped.

## What the performance panel reports

| Counter | Source |
| --- | --- |
| VM FPS | Presents / second on the main thread |
| Guest ops/s | Instruction retire count / wall time |
| WASM memory | `WebAssembly.Memory` byte length |
| Framebuffer | Width × height × 4 |
| Network RTT | Proxy round-trip of the last fetch |
| CPU util | Slice time / vsync interval, clamped 0–100% |

## Techniques in use

* Dirty bit on GPU SBI `present`
* Transferable `ArrayBuffer` for frames (SAB if available)
* Timesliced `vm_run(ops)` so net/input can preempt
* Bitmap font + fill/text SBI instead of per-pixel guest stores
* `opt-level = "s"` / LTO on the WASM crate
