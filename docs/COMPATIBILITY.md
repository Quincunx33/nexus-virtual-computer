# Compatibility

NEXUS does **not** claim full browser compatibility. The table is what has actually been reasoned about or exercised against the virtio-html rasterizer and the HTTP bridge.

| Website | Protocol | Loaded? | JavaScript? | Images? | CSS? | Cookies? | Storage? | Limitations |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `https://example.com` | HTTPS GET | Yes (target) | No | None on page | Color/typography lite | N/A | N/A | Static IANA page; this is the acceptance test |
| `https://example.org` | HTTPS GET | Yes | No | None | Lite | N/A | N/A | Same as example.com |
| `https://www.iana.org` | HTTPS GET | Partial | No | No (placeholders) | Lite | Session | No | Complex layout flattened to blocks |
| Wikipedia article | HTTPS GET | Partial | No | No | Lite | Session | No | Very long pages clip at 1.5 MiB; no wiki JS |
| `https://news.ycombinator.com` | HTTPS GET | Partial | No | N/A | Lite | Session | No | Tables become stacked text |
| Sites requiring JS to render | HTTPS GET | Shell only | **No** | — | — | — | — | Documented non-goal of this phase |
| HTTP on port ≠ 80/443 | — | No | — | — | — | — | — | Proxy policy |
| `ftp:`, `ws:`, `file:` | — | No | — | — | — | — | Not bridged |

The guest browser normalizes domain hostnames to HTTPS and sends whitespace-containing text or a single-token search term to DuckDuckGo HTML search. It keeps an eight-entry back/forward list, resolves ordinary absolute/root-relative/path-relative links, and supports wheel or arrow-key page scrolling. It remains a small static HTML/CSS rasterizer: search pages that require JavaScript, interactive forms, and modern dynamic sites still need a full host browser and are outside the guest engine's contract.

## Host browsers

| Host | WASM | Worker | WebGPU | WebGL2 | OPFS | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| Desktop Chrome | Required | Yes | Preferred | Fallback | Yes | Baseline |
| Desktop Firefox | Required | Yes | Yes (recent) | Fallback | Yes | |
| Desktop Safari | Required | Yes | Safari 18+ | Fallback | Partial | SAB often missing in iframes |
| iPadOS Safari | Required | Yes | Varies | Fallback | Partial | Touch + on-screen keys |
| Chrome Android | Required | Yes | Varies | Fallback | Yes | |

The start screen reports detected capabilities before power-on.

## Guest engine limits (virtio-html)

Supported-ish: `html, head, title, body, h1–h6, p, a, ul/ol/li, pre, code, br, hr, div, span, strong/b, em/i, img` (alt/placeholder), inline `style` color/background on body.

Not supported: JavaScript, CSS layout (flex/grid), webfonts, `<canvas>`, `<video>`, `<iframe>`, shadow DOM, XHR inside the page.
