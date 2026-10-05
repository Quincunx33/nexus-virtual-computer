# Security

The virtual machine is untrusted code. NexusOS and anything it fetches are treated as hostile.

## Capability table

| Capability | Guest has it? | Gate |
| --- | --- | --- |
| Read/write virtual disk blob | Yes | Isolated origin storage (OPFS / IndexedDB) |
| Host filesystem | No | Not in the SBI |
| Host process execution | No | Not in the SBI |
| `eval` on the host | No | Worker message schema is closed |
| Display | Yes | Framebuffer only |
| Keyboard / mouse | Yes | Only while the canvas is focused / captured |
| Clipboard | Optional | Requires a user gesture + Permissions API |
| Network | Optional | User toggle + SSRF-limited HTTP proxy |
| Arbitrary TCP | No | Impossible in-browser |

## Message validation

Main ↔ worker messages are tagged objects with a `type` discriminant. Unknown types are dropped. Numeric fields are range-checked before they enter WASM (key codes, mouse coords, buffer lengths).

## Memory

Guest loads/stores outside RAM, MMIO, and the framebuffer do not map onto host objects. WASM linear memory is the only backing; it cannot index the host JS heap.

## Network

See [NETWORKING.md](NETWORKING.md). The guest may request a URL; the proxy decides. Private networks are blocked to limit SSRF from a deployed origin.

## Persistence

Disk images never leave the origin unless the user explicitly exports them. Import replaces the image after a magic-header check (`NEXDISK1`).

## Origin isolation

There is no third-party cookie sharing with the host page. The guest cookie jar lives in worker memory for the session only.
