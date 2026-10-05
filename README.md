# NEXUS — Browser Virtual Computer

NEXUS is a small virtual computer that runs in a web page. Its display is produced by a real NexusOS guest running on the included RV32IM interpreter compiled to WebAssembly; it is not a host-side HTML mock. The guest includes a desktop, terminal, and static-page virtual browser.

## Run it

Requires Node.js 22 or newer.

```sh
npm ci
npm run dev
```

Open the app in a modern desktop or mobile browser. Select **Power on** and open Terminal or Browser from the guest desktop. On touchscreen devices, tap the command/URL field below the display to open the device keyboard; type a command or URL and press `Enter`/`Send`. The shortcut strip provides URL select, Tab, command/page arrows, and Escape. The input event buffer is sized for full pasted URLs, and IME composition is kept out of the guest until it is committed. In the guest Browser, tap **URL** to select the current address before typing a replacement. Hostnames are upgraded to HTTPS, phrases are sent to DuckDuckGo HTML search, and ordinary relative links resolve against the current page. Browser pages can be scrolled with a mouse wheel or the shortcut arrows. On desktop, click inside the virtual display and use the keyboard; `Ctrl+A`/`Command+A` selects the sample URL in the Browser. Press `Esc` from Terminal or Browser to return to the desktop. The guest shell supports `help`, `uname`, `whoami`, `pwd`, `ps`, `history`, `ls`, `cat`, `head`, `wc`, `dmesg`, `free`, `date`, `echo`, `touch`, `rm`, `fetch`, `open`, `write`, `clear`, and `reboot`; arrow keys recall history and edit the command line.

The project also provides `npm run typecheck`, `npm test`, and `npm run build` checks. `startup.sh` is an idempotent, non-blocking development-preview launcher.

## Virtual machine and storage

The browser downloads `public/nexus-vm.wasm`, instantiates it in a dedicated Web Worker, and runs the included `wasm/guest/nexus-os.elf` image on one RV32IM hart. The guest draws into a 960×600 RGBA framebuffer. Its 8 MiB `NEXDISK1` virtual disk is stored per-origin in IndexedDB; it never reads or writes the host filesystem.

The prebuilt WASM asset is checked into `public/` so the ordinary Node/Vercel build does not require a Rust toolchain. To rebuild it, install Rust with the `wasm32-unknown-unknown` target and run:

```sh
cd wasm
cargo build -p nexus-vm --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/nexus_vm.wasm ../public/nexus-vm.wasm
```

The VM crate uses the bundled guest ELF if the generated guest target is absent. To change the guest OS itself, build its `nexus-os` RISC-V target, then rebuild the VM crate.

## Network and browser limits

The guest Internet switch is on by default and can be disabled from the app. Guest HTTP(S) GETs go through the same-origin `/api/net/proxy` route, which blocks private/local IP ranges, nonstandard ports, embedded credentials, and unsafe redirects. Requests time out after 12 seconds and response bodies are limited to 1.5 MiB. This is an HTTP page bridge, not a VPN or raw TCP/UDP socket.

The guest browser renders a limited subset of HTML and CSS. It does not execute page JavaScript and does not support modern interactive websites, third-party cookies, arbitrary images, or full CSS layout. `docs/COMPATIBILITY.md`, `docs/NETWORKING.md`, and `docs/SECURITY.md` describe the current compatibility and security boundaries.
