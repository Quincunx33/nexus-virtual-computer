//! virtio-html: host-side HTML-lite rasterizer into the guest framebuffer.
//! Not a browser engine. No JavaScript. Documented in docs/COMPATIBILITY.md.

use crate::font;

#[derive(Clone)]
pub struct HitRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub href: String,
}

fn rgba(c: u32) -> [u8; 4] {
    [
        ((c >> 16) & 0xff) as u8,
        ((c >> 8) & 0xff) as u8,
        (c & 0xff) as u8,
        0xff,
    ]
}

fn fill_rect(fb: &mut [u8], stride: i32, fb_w: i32, fb_h: i32, x: i32, y: i32, w: i32, h: i32, c: [u8; 4]) {
    let x1 = x.max(0);
    let y1 = y.max(0);
    let x2 = (x + w).min(fb_w);
    let y2 = (y + h).min(fb_h);
    for py in y1..y2 {
        for px in x1..x2 {
            let i = ((py * stride + px) * 4) as usize;
            if i + 3 < fb.len() {
                fb[i] = c[0];
                fb[i + 1] = c[1];
                fb[i + 2] = c[2];
                fb[i + 3] = c[3];
            }
        }
    }
}

fn decode_entities(s: &str) -> String {
    s.replace("&nbsp;", " ")
        .replace("&quot;", "\u{22}")
        .replace("&#39;", "\u{27}")
        .replace("&apos;", "\u{27}")
        .replace("&lt;", "\u{3c}")
        .replace("&gt;", "\u{3e}")
        .replace("&amp;", "\u{26}")
}


fn parse_hex_color(s: &str) -> Option<u32> {
    let s = s.trim();
    let hex = s.strip_prefix('#').unwrap_or(s);
    if hex.len() == 3 {
        let r = u32::from_str_radix(&hex[0..1], 16).ok()?;
        let g = u32::from_str_radix(&hex[1..2], 16).ok()?;
        let b = u32::from_str_radix(&hex[2..3], 16).ok()?;
        return Some((r << 20) | (r << 16) | (g << 12) | (g << 8) | (b << 4) | b);
    }
    if hex.len() == 6 {
        return u32::from_str_radix(hex, 16).ok();
    }
    None
}

enum Tok {
    Text(String),
    Open(String, String),
    Close(String),
}

fn tokenize(html: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let bytes = html.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            if let Some(end) = html[i..].find('>') {
                let inner = html[i + 1..i + end].trim();
                if inner.starts_with("!--") {
                    i += end + 1;
                    continue;
                }
                if inner.starts_with('/') {
                    let name = inner[1..]
                        .split_whitespace()
                        .next()
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    out.push(Tok::Close(name));
                } else {
                    let mut parts = inner.splitn(2, char::is_whitespace);
                    let name = parts
                        .next()
                        .unwrap_or("")
                        .trim_end_matches('/')
                        .to_ascii_lowercase();
                    let rest = parts.next().unwrap_or("").to_string();
                    out.push(Tok::Open(name, rest));
                }
                i += end + 1;
                continue;
            }
        }
        let rest = &html[i..];
        if let Some(p) = rest.find('<') {
            if p > 0 {
                out.push(Tok::Text(rest[..p].to_string()));
            }
            i += p;
        } else {
            out.push(Tok::Text(rest.to_string()));
            break;
        }
    }
    out
}


fn attr(attrs: &str, key: &str) -> Option<String> {
    let key_eq = format!("{key}=");
    let lower = attrs.to_ascii_lowercase();
    let pos = lower.find(&key_eq)?;
    let rest = &attrs[pos + key_eq.len()..].trim_start();
    if rest.starts_with('"') {
        let end = rest[1..].find('"')?;
        return Some(rest[1..1 + end].to_string());
    }
    if rest.starts_with('\'') {
        let end = rest[1..].find('\'')?;
        return Some(rest[1..1 + end].to_string());
    }
    Some(rest.split_whitespace().next()?.trim_end_matches('>').to_string())
}

pub fn render(
    html: &[u8],
    fb: &mut [u8],
    fb_w: i32,
    fb_h: i32,
    ox: i32,
    oy: i32,
    width: i32,
    height: i32,
    scroll_y: i32,
) -> Vec<HitRect> {
    let text = match std::str::from_utf8(html) {
        Ok(s) => s,
        Err(_) => std::str::from_utf8(&html[..html.iter().take_while(|b| **b < 128).count()]).unwrap_or(""),
    };
    let mut bg = 0xf0f0f2u32;
    let mut fg = 0x222222u32;
    if let Some(idx) = text.to_ascii_lowercase().find("background") {
        let slice = &text[idx..text.len().min(idx + 80)];
        if let Some(hash) = slice.find('#') {
            if let Some(c) = parse_hex_color(&slice[hash..slice.len().min(hash + 7)]) {
                bg = c;
            }
        }
    }
    fill_rect(fb, fb_w, fb_w, fb_h, ox, oy, width, height, rgba(bg));

    let toks = tokenize(text);
    let mut hits = Vec::new();
    let mut x = ox + 20;
    let mut y = oy + 16 - scroll_y.max(0);
    let max_x = ox + width - 20;
    let max_y = oy + height - 8;
    let mut scale = 1i32;
    let mut skip = false;
    let mut in_link: Option<String> = None;
    let mut link_x = x;
    let mut heading = false;
    let mut bullet = false;
    let line_h = |sc: i32| 10 * sc + 6;

    let mut wrap_nl = |x: &mut i32, y: &mut i32, sc: i32| {
        *x = ox + 20;
        *y += line_h(sc);
    };

    for tok in toks {
        if y > max_y {
            break;
        }
        match tok {
            Tok::Open(name, attrs) => match name.as_str() {
                "script" | "style" | "head" | "noscript" => skip = true,
                "br" => {
                    x = ox + 20;
                    y += line_h(scale);
                }
                "p" | "div" => {
                    if x != ox + 20 {
                        x = ox + 20;
                        y += line_h(scale);
                    }
                    y += 4;
                }
                "h1" => {
                    heading = true;
                    scale = 3;
                    x = ox + 20;
                    y += 10;
                }
                "h2" => {
                    heading = true;
                    scale = 2;
                    x = ox + 20;
                    y += 8;
                }
                "h3" | "h4" | "h5" | "h6" => {
                    heading = true;
                    scale = 2;
                    x = ox + 20;
                    y += 6;
                }
                "li" => {
                    bullet = true;
                    x = ox + 28;
                    y += line_h(1);
                    font::blit_text(fb, fb_w, fb_w, fb_h, ox + 16, y, b"*", rgba(fg), 1);
                }
                "hr" => {
                    y += 8;
                    fill_rect(fb, fb_w, fb_w, fb_h, ox + 20, y, width - 40, 1, rgba(0xcccccc));
                    y += 8;
                    x = ox + 20;
                }
                "a" => {
                    in_link = attr(&attrs, "href");
                    link_x = x;
                }
                "img" => {
                    let alt = attr(&attrs, "alt").unwrap_or_else(|| "image".into());
                    let box_w = (width - 40).min(240);
                    let box_h = 48;
                    fill_rect(fb, fb_w, fb_w, fb_h, x, y, box_w, box_h, rgba(0xdddddd));
                    font::blit_text(
                        fb,
                        fb_w,
                        fb_w,
                        fb_h,
                        x + 8,
                        y + 16,
                        format!("[img] {alt}").as_bytes(),
                        rgba(0x555555),
                        1,
                    );
                    y += box_h + 8;
                    x = ox + 20;
                }
                "title" => skip = true,
                _ => {}
            },
            Tok::Close(name) => match name.as_str() {
                "script" | "style" | "head" | "noscript" | "title" => skip = false,
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    heading = false;
                    scale = 1;
                    x = ox + 20;
                    y += line_h(2);
                }
                "p" | "div" => {
                    x = ox + 20;
                    y += line_h(scale) + 4;
                }
                "li" => bullet = false,
                "a" => {
                    if let Some(href) = in_link.take() {
                        let w = (x - link_x).max(8);
                        hits.push(HitRect {
                            x: link_x,
                            y,
                            w,
                            h: line_h(scale),
                            href,
                        });
                    }
                }
                _ => {}
            },
            Tok::Text(raw) => {
                if skip {
                    continue;
                }
                let decoded = decode_entities(&raw);

                let collapsed = if heading {
                    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
                } else {
                    decoded
                        .split('\n')
                        .map(|l| l.trim())
                        .filter(|l| !l.is_empty())
                        .collect::<Vec<_>>()
                        .join(" ")
                };
                if collapsed.is_empty() {
                    continue;
                }
                let color = if in_link.is_some() { rgba(0x38488f) } else { rgba(fg) };
                let adv = 8 * scale;
                for word in collapsed.split(' ') {
                    if word.is_empty() {
                        continue;
                    }
                    let wpx = word.len() as i32 * adv;
                    if x + wpx > max_x && x > ox + 20 {
                        wrap_nl(&mut x, &mut y, scale);
                        if y > max_y {
                            break;
                        }
                    }
                    font::blit_text(fb, fb_w, fb_w, fb_h, x, y, word.as_bytes(), color, scale);
                    x += wpx + adv;
                }
                let _ = (heading, bullet);
            }
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_domain_paints_title() {
        let html = b"<html><head><title>x</title><style>body{background:#f0f0f2}</style></head><body><div><h1>Example Domain</h1><p>Hello.</p><p><a href=\"https://www.iana.org/domains/example\">More information...</a></p></div></body></html>";
        let mut fb = vec![0u8; 320 * 240 * 4];
        let hits = render(html, &mut fb, 320, 240, 0, 0, 320, 240, 0);
        assert!(!hits.is_empty());
        assert!(hits[0].href.contains("iana.org"));
        assert!(fb.iter().any(|&b| b != 0));
    }

    #[test]
    fn common_entities_are_decoded_without_reinterpreting_escaped_markup() {
        assert_eq!(decode_entities("A &amp; B &lt;tag&gt; &quot;x&quot;"), "A & B <tag> \"x\"");
    }

    #[test]
    fn page_scroll_moves_link_hit_targets_up_with_the_rendered_content() {
        let html = b"<body><p>first line</p><p>second line</p><p><a href=\"https://example.org/next\">next page</a></p></body>";
        let mut top = vec![0u8; 320 * 240 * 4];
        let mut scrolled = vec![0u8; 320 * 240 * 4];
        let top_hits = render(html, &mut top, 320, 240, 0, 0, 320, 240, 0);
        let scrolled_hits = render(html, &mut scrolled, 320, 240, 0, 0, 320, 240, 40);
        assert_eq!(top_hits.len(), 1);
        assert_eq!(scrolled_hits.len(), 1);
        assert_eq!(top_hits[0].y - scrolled_hits[0].y, 40);
    }
}
