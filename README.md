<div align="center">

# NEXUS
### A Virtual Computer. Inside Your Browser.

**A real RISC-V virtual machine running NexusOS — powered by WebAssembly, rendered in your browser.**

[![RISC-V](https://img.shields.io/badge/Architecture-RV32IM-6f42c1?style=for-the-badge)](https://en.wikipedia.org/wiki/RISC-V)
[![WebAssembly](https://img.shields.io/badge/Runtime-WebAssembly-654FF0?style=for-the-badge&logo=webassembly&logoColor=white)](https://webassembly.org/)
[![React](https://img.shields.io/badge/UI-React_19-149eca?style=for-the-badge&logo=react&logoColor=white)](https://react.dev/)
[![License](https://img.shields.io/badge/License-See_Repository-lightgrey?style=for-the-badge)](LICENSE)

[**Launch NEXUS**](https://nexus-virtual-computer.pages.dev/) · [Report a Bug](https://github.com/Quincunx33/nexus-virtual-computer/issues) · [Explore the Source](https://github.com/Quincunx33/nexus-virtual-computer)

</div>

---

## What is NEXUS?

NEXUS is an experimental virtual computer that runs inside a web browser. It boots a real **NexusOS guest** on an **RV32IM RISC-V interpreter compiled to WebAssembly**.

This is not an HTML mock desktop pretending to be an operating system. The guest OS executes inside the virtual machine, draws to a framebuffer, and receives keyboard and pointer input through a dedicated Web Worker.

Power it on, open the guest terminal, explore its virtual filesystem, or browse simple static web pages — all without installing a desktop emulator.

## Inside the machine

```text
┌──────────────────────────────────────────────────────┐
│                    NEXUS WEB APP                     │
│                                                      │
│   Desktop UI  ·  Power Controls  ·  Keyboard Input   │
│                         │                            │
│                    Web Worker                        │
│                         │                            │
│                 nexus-vm.wasm                        │
│                  RV32IM CPU                           │
│                         │                            │
│                    NexusOS                           │
│             Terminal · Virtual Browser               │
│                         │                            │
│              960 × 600 Framebuffer                    │
│                         │                            │
│               Virtual Disk (8 MiB)                    │
│             IndexedDB · Per-Origin                    │
└──────────────────────────────────────────────────────┘
```

The VM runs on one RV32IM hart. Its prebuilt WebAssembly binary is served from `public/nexus-vm.wasm`, while the guest image is included at `wasm/guest/nexus-os.elf`.

## Features

| Feature | Details |
| --- | --- |
| **Real guest OS** | NexusOS running on an RV32IM RISC-V virtual CPU |
| **WebAssembly runtime** | VM executes in a dedicated Web Worker |
| **Framebuffer display** | Guest renders to a 960 × 600 RGBA framebuffer |
| **Virtual storage** | 8 MiB `NEXDISK1` disk persisted per origin in IndexedDB |
| **Guest desktop** | Power-on flow with Terminal and Browser apps |
| **Command-line shell** | Filesystem, system information, history and basic file commands |
| **Virtual browser** | HTTP(S) page bridge with a lightweight HTML/CSS renderer |
| **Touch support** | On-screen input field, URL controls and keyboard shortcuts |
| **Network safety** | Optional HTTP bridge with URL, port, redirect and response-size restrictions |
| **Host isolation** | Guest cannot access the host filesystem or execute host processes |

## Try it

Open the deployed app:

**[https://nexus-virtual-computer.pages.dev/](https://nexus-virtual-computer.pages.dev/)**

1. Open NEXUS in a modern browser.
2. Select **Power on**.
3. From the guest desktop, open **Terminal** or **Browser**.
4. On desktop, click the virtual display to send keyboard input.
5. On touch devices, use the input field below the display and its shortcut strip.

In the guest Browser, use **URL** to select the current address before entering another one. Hostnames are upgraded to HTTPS; search phrases are sent to DuckDuckGo HTML search.

## Terminal commands

The guest shell currently supports:

```sh
help
uname
whoami
pwd
ps
history
ls
cat
head
wc
dmesg
free
date
echo
touch
rm
fetch
open
write
clear
reboot
```

Use the arrow keys to navigate command history and edit the command line. Press `Esc` from Terminal or Browser to return to the guest desktop.

## Run locally

### Requirements

- Node.js 22+
- npm
- A modern browser with WebAssembly and Web Worker support

### Install and start

```bash
git clone https://github.com/Quincunx33/nexus-virtual-computer.git
cd nexus-virtual-computer

npm ci
npm run dev
```

Open the local address printed by Vite (the development server uses port **8080**).

### Useful scripts

| Command | Purpose |
| --- | --- |
| `npm run dev` | Start the development server |
| `npm run build` | Build the application and run database migrations |
| `npm run build:dev` | Create a development-mode build |
| `npm run preview` | Preview the production build |
| `npm run typecheck` | Run TypeScript checks |
| `npm test` | Run the project test suites |
| `npm run lint` | Run ESLint |
| `npm run db:migrate` | Run database migrations |

## Rebuilding the WebAssembly VM

The repository includes a prebuilt WASM binary, so the normal application build does **not** require Rust.

To rebuild the VM binary, install Rust and the `wasm32-unknown-unknown` target, then run:

```bash
cd wasm
rustup target add wasm32-unknown-unknown
cargo build -p nexus-vm --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/nexus_vm.wasm ../public/nexus-vm.wasm
```

The VM crate uses the bundled guest ELF when the generated guest target is absent. To modify NexusOS itself, build its RISC-V guest target first, then rebuild the VM crate.

## Networking: what it can and cannot do

NEXUS uses a browser-compatible HTTP bridge. Guest requests travel through the VM and Worker to the same-origin `/api/net/proxy` route, which performs the external HTTP request.

**Supported**
- HTTP and HTTPS requests through the proxy
- GET, HEAD and POST
- Limited redirect handling with policy checks
- A session cookie jar in Worker memory

**Not supported**
- Arbitrary raw TCP or UDP sockets
- ICMP, FTP, WebSocket-to-any-host or a general VPN tunnel
- JavaScript execution inside guest browser pages
- Full modern browser layout, interactive websites or third-party cookies

The proxy rejects private/local destinations, embedded credentials and nonstandard ports, and applies a 12-second timeout and a 1.5 MiB response-body limit. The guest browser is a lightweight static HTML/CSS renderer, not a replacement for Chrome or Firefox.

For the detailed compatibility matrix and network design, see:
- [Browser compatibility](docs/COMPATIBILITY.md)
- [Networking architecture](docs/NETWORKING.md)
- [Security model](docs/SECURITY.md)

## Security and isolation

The guest is treated as untrusted code.

- Guest memory is backed by WebAssembly linear memory.
- Host filesystem access and host process execution are not exposed to the guest.
- Worker messages use tagged message types and validate numeric input.
- Virtual disks are isolated to the current browser origin.
- Network access is optional and passes through the restricted HTTP proxy.
- Disk export requires an explicit user action.

Read the [security notes](docs/SECURITY.md) before exposing a deployment to untrusted users.

## Technology

| Layer | Technology |
| --- | --- |
| Application UI | React 19 |
| App framework / routing | TanStack React Start |
| Build tooling | Vite |
| Server deployment | Nitro |
| Virtual machine | Rust → WebAssembly |
| Guest architecture | RISC-V RV32IM |
| Guest display | RGBA framebuffer |
| Persistent virtual disk | IndexedDB |
| Styling | Tailwind CSS |

## Project layout

```text
.
├── public/
│   └── nexus-vm.wasm       # Prebuilt WebAssembly virtual machine
├── wasm/
│   └── guest/
│       └── nexus-os.elf    # NexusOS guest image
├── src/                    # Application and runtime integration
├── server/                 # Server routes and middleware
├── scripts/                # Build, migration and development utilities
├── docs/
│   ├── COMPATIBILITY.md    # Host and guest-browser compatibility
│   ├── NETWORKING.md       # HTTP bridge and network boundaries
│   └── SECURITY.md         # Isolation and security model
└── package.json
```

## Contributing

Ideas, bug reports and improvements are welcome.

1. Open an issue describing the bug or proposed change.
2. Keep changes focused and document any new runtime limitations.
3. Run the relevant checks before submitting a pull request:

```bash
npm run typecheck
npm test
npm run lint
npm run build
```

For VM or guest-OS changes, include the relevant Rust build steps and explain how the change was verified.

## Project status

NEXUS is an evolving browser-based virtual computer. Its RISC-V guest runtime, desktop, terminal, virtual disk and limited static-page browser are implemented, but it is **not** a general-purpose PC emulator or a full web browser.

See the documentation above for the tested compatibility scope and known limitations.

---

<div align="center">

**NEXUS — A computer, one browser tab away.**

Built by [Quincunx33](https://github.com/Quincunx33).

</div>
