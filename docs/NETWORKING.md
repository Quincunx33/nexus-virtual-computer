# Networking

## The hard limit

A browser tab cannot open arbitrary TCP or UDP sockets. WebAssembly running in that tab cannot either. NEXUS does not pretend otherwise.

```
Virtual browser  →  NexusOS syscall (ecall NET_TX)
                 →  Virtio-style NIC in the WASM VM
                 →  Worker posts a request record
                 →  Same-origin POST /api/net/proxy   (or direct fetch for allowlisted CORS)
                 →  Host fetch() on the server
                 →  Real http/https origin
                 →  Response bytes into guest RAM
                 →  virtio-html raster into the framebuffer
```

## What can reach the public Internet

| Traffic | Supported? | How |
| --- | --- | --- |
| HTTP/1.1 GET/HEAD/POST | Yes | Server-side `fetch` proxy |
| HTTPS | Yes | As above (TLS terminates at the host, not in the guest) |
| Cookies | Session jar in the worker | `Cookie` / `Set-Cookie` forwarded; not persisted by default |
| Redirects | Yes, max 4 | Each hop re-checked against the public-URL policy |
| Raw TCP | No | No API |
| Raw UDP / ICMP | No | No API |
| TLS inside the guest | No | Would need a guest TCP+TLS stack *and* a byte-stream tunnel |
| WebSocket to arbitrary hosts | No | Would need a WS-to-TCP relay we do not run |
| WebTransport | Detected, unused | Same as TCP: no generic server |
| WebRTC DataChannel | Detected, unused | Only useful with a cooperating peer |

## Failure mode

If the user disables the internet bridge, `navigator.onLine` is false, or the proxy rejects a URL, the guest receives `NETWORK UNAVAILABLE` and does not crash.

## Proxy policy (SSRF)

The proxy is the only way out. It:

* allows `http:` and `https:` only
* rejects localhost, RFC1918, link-local, ULA, metadata IPs (`169.254.169.254`, `fd00:ec2::254`)
* rejects non-default ports other than 80/443
* caps response bodies (1.5 MiB)
* times out (12 s)
* does not follow redirects onto private literals

This is **not** a general TCP VPN. It is an application-layer HTTP bridge, which is the only standards-compliant path to `https://example.com` from a browser sandbox.
