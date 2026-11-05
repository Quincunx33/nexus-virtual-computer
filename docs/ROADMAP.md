# NEXUS — Product Roadmap

> **Status:** Proposed roadmap  
> **Scope:** Forward-looking product plan, not a claim that these features are already implemented.  
> **Planning rule:** Stabilize and verify each layer before expanding the next one. Items may be reordered as technical constraints and test results become clearer.

NEXUS is a browser-hosted RISC-V virtual computer. The roadmap focuses on making the existing guest runtime more dependable, improving the virtual machine experience, and gradually expanding NexusOS and its lightweight guest browser.

## Current baseline

The repository currently documents these capabilities:

- A single-hart RV32IM guest runtime compiled to WebAssembly.
- A dedicated Web Worker for VM execution.
- NexusOS guest image with a desktop, terminal and lightweight browser.
- A 960 × 600 RGBA framebuffer.
- An 8 MiB per-origin virtual disk persisted through IndexedDB.
- A restricted HTTP(S) bridge; no arbitrary raw TCP/UDP sockets.
- A deliberately limited static HTML/CSS renderer; no guest-page JavaScript engine.

These are the baseline, not roadmap promises. See [Compatibility](COMPATIBILITY.md), [Networking](NETWORKING.md) and [Security](SECURITY.md) for current boundaries.

## Roadmap at a glance

| Phase | Focus | Priority |
| --- | --- | --- |
| 0 | Reliability, deployment and repeatable verification | Highest |
| 1 | Virtual machine correctness and diagnostics | High |
| 2 | Virtual hardware, input and display improvements | High |
| 3 | Virtual disk, files and session management | High |
| 4 | NexusOS desktop and shell experience | Medium |
| 5 | Guest browser rendering and usability | Medium |
| 6 | Networking and safe connectivity extensions | Medium |
| 7 | Performance, portability and developer tooling | Ongoing |

---

## Phase 0 — Reliability and deployment

**Goal:** Make the existing experience build, deploy and start consistently before adding major features.

- [ ] Verify production builds on the supported deployment targets, including Cloudflare Pages and Vercel.
- [ ] Confirm Nitro output, server routes and static WASM asset delivery on each target.
- [ ] Add a deployment smoke test for the app shell, VM binary, guest image and /api/net/proxy route.
- [ ] Verify database configuration paths: configured Neon (DATABASE_URL) and local/preview PGLite fallback.
- [ ] Make missing optional environment variables produce clear, actionable diagnostics.
- [ ] Add a repeatable end-to-end boot test: load app → power on → guest desktop → terminal → reboot.
- [ ] Record browser/device test results instead of relying on build success alone.
- [ ] Document known deployment limitations and supported environment variables.

**Exit criteria:** A clean build is not enough. A deployed instance must boot the guest, load its WASM asset, accept input, render frames and fail gracefully when optional services are unavailable.

## Phase 1 — VM correctness and diagnostics

**Goal:** Turn the VM core into a testable and observable execution engine.

- [ ] Create RV32IM instruction-level tests using known input/output register and memory states.
- [ ] Validate instruction decoding, arithmetic edge cases, alignment and illegal-instruction behavior.
- [ ] Add tests for traps, exceptions and guest fault reporting where supported by the current machine model.
- [ ] Define and document the guest memory map and MMIO register contract.
- [ ] Test boundary conditions for RAM, framebuffer and device access.
- [ ] Add deterministic VM reset and power-cycle tests.
- [ ] Add a structured diagnostic channel for guest faults, VM stops and worker errors.
- [ ] Add optional debug counters: instructions executed, frames rendered, worker messages and VM uptime.
- [ ] Establish a reproducible performance baseline for boot time and frame delivery.

**Exit criteria:** Core instruction and memory behavior is covered by automated tests, and guest failures produce actionable diagnostics rather than silent hangs.

## Phase 2 — Virtual hardware and input/display

**Goal:** Make the virtual machine feel more like a coherent computer instead of a framebuffer demo.

- [ ] Formalize virtual device interfaces and version their host/guest message contracts.
- [ ] Improve keyboard handling: modifiers, key-up reliability, focus recovery and layout consistency.
- [ ] Improve pointer handling: coordinate scaling, pointer capture and touch-to-pointer mapping.
- [ ] Add display resizing/scaling without changing guest framebuffer semantics unexpectedly.
- [ ] Explore configurable framebuffer resolutions after the current 960 × 600 path is stable.
- [ ] Add a virtual clock/timer device if required by the guest scheduler and applications.
- [ ] Add a serial/debug console for boot logs and recovery diagnostics.
- [ ] Evaluate audio output as an optional device, gated by browser support and user permission.

**Exit criteria:** Input remains reliable across desktop and touch browsers; device behavior is documented and covered by integration tests.

## Phase 3 — Virtual disk and session management

**Goal:** Make guest data safer, more portable and easier to manage.

- [ ] Add explicit disk usage and storage status in the host UI.
- [ ] Implement validated disk export and import with format/version checks.
- [ ] Add downloadable backup files and clear restore confirmation.
- [ ] Add named snapshots or checkpoints if the disk format can support safe, atomic snapshots.
- [ ] Handle quota errors, unavailable IndexedDB and interrupted writes without corrupting the active disk.
- [ ] Add disk format versioning and migration rules.
- [ ] Provide a safe reset/wipe flow with a clear warning.
- [ ] Test persistence across reloads, browser restarts and multiple tabs.

**Exit criteria:** Users can back up and restore their virtual disk, and storage failures never silently destroy the only copy of guest data.

## Phase 4 — NexusOS desktop and shell

**Goal:** Grow the guest OS into a useful small environment.

- [ ] Improve shell parsing, quoting, command errors and help output.
- [ ] Expand filesystem operations and add predictable directory navigation.
- [ ] Add a basic text editor with save/open workflows.
- [ ] Add process/task visibility and clearer system information.
- [ ] Improve desktop window management and application lifecycle.
- [ ] Add guest settings for display, input and network preferences.
- [ ] Add a file manager backed only by the virtual disk.
- [ ] Add a small set of bundled guest utilities and sample files.
- [ ] Define a stable application launch/exit contract for future guest apps.

**Exit criteria:** Core guest workflows—files, editing, app switching, settings and reboot—work consistently and persist as intended.

## Phase 5 — Guest browser improvements

**Goal:** Improve the lightweight static browser while keeping its scope honest.

- [ ] Expand HTML parsing and malformed-markup recovery.
- [ ] Implement a small, documented CSS box model and block/inline layout rules.
- [ ] Add basic image decoding/rendering for a safe, explicitly supported set of formats.
- [ ] Improve text wrapping, font fallback, lists, tables and long-page scrolling.
- [ ] Add clear loading, timeout, blocked-request and unsupported-content states.
- [ ] Improve back/forward navigation and URL handling.
- [ ] Add a small compatibility test corpus with deterministic fixture pages.
- [ ] Document unsupported HTML/CSS features in the compatibility matrix.
- [ ] Evaluate a restricted script model only as a separate, security-reviewed project; do not imply full JavaScript compatibility.

**Exit criteria:** Supported static pages render predictably, unsupported features fail visibly, and compatibility claims are backed by fixtures.

## Phase 6 — Networking and safe connectivity

**Goal:** Improve the existing HTTP bridge without pretending a browser tab has unrestricted socket access.

- [ ] Add explicit request/response tests for methods, headers, redirects and body limits.
- [ ] Strengthen DNS/IP validation and re-check every redirect target.
- [ ] Add clearer user controls for enabling/disabling guest network access.
- [ ] Improve guest-visible network error codes and diagnostics.
- [ ] Add bounded caching for safe GET responses where it improves usability.
- [ ] Define cookie/session behavior and provide a way to clear the guest session.
- [ ] Add configurable request limits with conservative deployment defaults.
- [ ] Evaluate optional WebSocket or other relay support only if a separately deployed, authenticated relay is designed and threat-modeled.

**Exit criteria:** Network behavior is documented, bounded, tested against SSRF cases and transparent to the user. Raw TCP/UDP is not promised by the browser-only architecture.

## Phase 7 — Performance, portability and developer tooling

**Goal:** Keep NEXUS responsive and make contributions easier as the project grows.

- [ ] Profile VM execution, framebuffer copies and worker message overhead.
- [ ] Reduce unnecessary frame transfers and UI re-renders.
- [ ] Benchmark low-memory mobile devices and define graceful fallback behavior.
- [ ] Test current supported host browsers and maintain a versioned compatibility matrix.
- [ ] Add CI jobs for typecheck, tests, lint, production build and Rust/WASM reproducibility.
- [ ] Add artifact checks to detect stale or mismatched prebuilt WASM binaries.
- [ ] Document the guest build toolchain and one-command rebuild workflow.
- [ ] Add architecture notes for VM, worker, guest ABI, storage and networking.
- [ ] Add a diagnostics bundle users can copy when reporting a bug, excluding secrets and private browsing data.

**Exit criteria:** Contributors can reproduce builds and tests, and performance regressions can be measured rather than guessed.

---

## Non-goals and architectural boundaries

The following are not assumed deliverables of the browser-only roadmap:

- Direct access to the user's host filesystem or host processes.
- Unrestricted raw TCP/UDP sockets from a browser tab.
- A full Chrome/Firefox-class browser engine.
- Running arbitrary host executables or native desktop applications.
- Guaranteed WebGPU, SharedArrayBuffer or persistent storage availability on every browser.
- Cloud synchronization or multi-user accounts without a separately designed backend and privacy model.

These may require a separate native companion, remote execution service or substantial security architecture. They should not be represented as ordinary browser features.

## Definition of done

A roadmap item is complete only when:

1. Its behavior and limitations are documented.
2. Relevant automated tests are added and pass.
3. Typecheck, lint and production build pass where applicable.
4. Browser/device integration is verified for affected features.
5. Security and data-loss implications are reviewed.
6. The README and compatibility documentation reflect the actual shipped behavior.

**Roadmap principle:** correctness first, capabilities second, polish continuously.
