#![no_std]
#![no_main]

mod sbi;

use sbi::{self as s};

core::arch::global_asm!(
    ".section .text.init,\"ax\",@progbits",
    ".globl _start",
    "_start:",
    "    la sp, _stack_top",
    "    call rust_main",
    "1:  j 1b",
);

const COL_BG: u32 = 0x12151c;
const COL_TOP: u32 = 0x1c212b;
const COL_PANEL: u32 = 0x232833;
const COL_ICON: u32 = 0x2c3340;
const COL_ICON_H: u32 = 0x3a4354;
const COL_FG: u32 = 0xe8eaee;
const COL_MUTED: u32 = 0x9aa1ad;
const COL_ACCENT: u32 = 0xb9c2ce;
const COL_TERM_BG: u32 = 0x0c0e12;
const COL_TERM_FG: u32 = 0xd6dbe3;
const COL_BTN: u32 = 0x3d4554;
const COL_URL: u32 = 0x151820;
const COL_RED: u32 = 0xc97a76;

const MODE_DESKTOP: i32 = 0;
const MODE_TERM: i32 = 1;
const MODE_BROWSER: i32 = 2;

const KEY_ENTER: u32 = 13;
const KEY_BS: u32 = 8;
const KEY_ESC: u32 = 27;
const KEY_LEFT: u32 = 0x80;
const KEY_RIGHT: u32 = 0x81;
const KEY_UP: u32 = 0x82;
const KEY_DOWN: u32 = 0x83;
const KEY_SELECT_ALL: u32 = 0x84;
const KEY_HOME: u32 = 0x85;
const KEY_END: u32 = 0x86;
const KEY_DELETE: u32 = 0x87;
const KEY_PAGE_UP: u32 = 0x88;
const KEY_PAGE_DOWN: u32 = 0x89;
const KEY_TAB: u32 = 9;
const TERM_HISTORY_CAP: usize = 12;

struct State {
    mode: i32,
    fb_w: i32,
    fb_h: i32,
    prev_btn: u32,
    boot_done: bool,
    term_lines: [[u8; 96]; 22],
    term_n: [u8; 22],
    term_len: usize,
    term_in: [u8; 96],
    term_in_len: usize,
    term_cursor: usize,
    term_history: [[u8; 96]; TERM_HISTORY_CAP],
    term_history_len: [u8; TERM_HISTORY_CAP],
    term_history_n: usize,
    term_history_i: usize,
    url: [u8; 256],
    url_len: usize,
    url_focus: bool,
    page: [u8; 65536],
    page_len: usize,
    loading: bool,
    history: [[u8; 256]; 8],
    hist_len: [u16; 8],
    hist_n: usize,
    hist_i: usize,
    page_scroll: i32,
    status: [u8; 64],
    status_len: usize,
}

static mut STATE: State = State {
    mode: MODE_DESKTOP,
    fb_w: 960,
    fb_h: 600,
    prev_btn: 0,
    boot_done: false,
    term_lines: [[0; 96]; 22],
    term_n: [0; 22],
    term_len: 0,
    term_in: [0; 96],
    term_in_len: 0,
    term_cursor: 0,
    term_history: [[0; 96]; TERM_HISTORY_CAP],
    term_history_len: [0; TERM_HISTORY_CAP],
    term_history_n: 0,
    term_history_i: 0,
    url: [0; 256],
    url_len: 0,
    url_focus: true,
    page: [0; 65536],
    page_len: 0,
    loading: false,
    history: [[0; 256]; 8],
    hist_len: [0; 8],
    hist_n: 0,
    hist_i: 0,
    page_scroll: 0,
    status: [0; 64],
    status_len: 0,
};

fn st() -> &'static mut State {
    unsafe { &mut *core::ptr::addr_of_mut!(STATE) }
}


fn copy_str(dst: &mut [u8], src: &str) -> usize {
    let b = src.as_bytes();
    let n = b.len().min(dst.len());
    dst[..n].copy_from_slice(&b[..n]);
    n
}

fn eq(a: &[u8], b: &str) -> bool {
    a == b.as_bytes()
}

fn starts(a: &[u8], b: &str) -> bool {
    let bb = b.as_bytes();
    a.len() >= bb.len() && &a[..bb.len()] == bb
}

fn set_status(s: &str) {
    let stt = st();
    stt.status_len = copy_str(&mut stt.status, s);
}

fn status_ref() -> &'static str {
    let stt = st();
    core::str::from_utf8(&stt.status[..stt.status_len]).unwrap_or("")
}

fn url_ref() -> &'static str {
    let stt = st();
    core::str::from_utf8(&stt.url[..stt.url_len]).unwrap_or("")
}

fn push_term(line: &str) {
    let stt = st();
    if stt.term_len == 22 {
        for i in 0..21 {
            stt.term_lines[i] = stt.term_lines[i + 1];
            stt.term_n[i] = stt.term_n[i + 1];
        }
        stt.term_len = 21;
    }
    let i = stt.term_len;
    let n = copy_str(&mut stt.term_lines[i], line);
    stt.term_n[i] = n as u8;
    stt.term_len += 1;
}

fn itoa(mut n: u32, buf: &mut [u8]) -> usize {
    if n == 0 {
        buf[0] = b'0';
        return 1;
    }
    let mut tmp = [0u8; 10];
    let mut i = 0;
    while n > 0 && i < 10 {
        tmp[i] = b'0' + (n % 10) as u8;
        n /= 10;
        i += 1;
    }
    let mut j = 0;
    while i > 0 {
        i -= 1;
        buf[j] = tmp[i];
        j += 1;
    }
    j
}

fn clock_text(buf: &mut [u8]) -> usize {
    let sec = s::time_ms() / 1000;
    let h = (sec / 3600) % 24;
    let m = (sec / 60) % 60;
    buf[0] = b'0' + (h / 10) as u8;
    buf[1] = b'0' + (h % 10) as u8;
    buf[2] = b':';
    buf[3] = b'0' + (m / 10) as u8;
    buf[4] = b'0' + (m % 10) as u8;
    5
}

fn clicked(m: &s::Mouse, x: i32, y: i32, w: i32, h: i32, prev: u32) -> bool {
    m.buttons & 1 != 0 && prev & 1 == 0 && m.x >= x && m.x < x + w && m.y >= y && m.y < y + h
}

fn boot() {
    s::log("Booting virtual machine...\n");
    let (w, h) = s::fb_size();
    let stt = st();
    if w > 0 {
        stt.fb_w = w;
        stt.fb_h = h;
    }
    let w = stt.fb_w;
    let h = stt.fb_h;
    s::fill(0, 0, w, h, 0x0b0c10);
    s::text(40, 48, "NEXUS", COL_FG, 3);
    s::text(40, 90, "Booting virtual machine...", COL_MUTED, 1);
    s::present();
    s::yield_frame();

    s::log("Initializing CPU...\n");
    s::text(40, 112, "Initializing CPU...        RV32IM hart 0", COL_MUTED, 1);
    s::present();
    s::yield_frame();

    s::log("Initializing memory...\n");
    s::text(40, 128, "Initializing memory...     RAM mapped 0x80000000", COL_MUTED, 1);
    s::present();
    s::yield_frame();

    s::log("Mounting disk...\n");
    s::text(40, 144, "Mounting disk...           NEXDISK1", COL_MUTED, 1);
    s::present();
    s::yield_frame();

    s::log("Starting kernel...\n");
    s::text(40, 160, "Starting kernel...         NexusOS 0.1", COL_MUTED, 1);
    s::present();
    s::yield_frame();

    s::log("Starting userspace...\n");
    s::text(40, 176, "Starting userspace...      init, wm, sh, web", COL_MUTED, 1);
    s::present();
    s::yield_frame();

    s::text(40, 208, "Ready.", COL_FG, 1);
    s::present();
    s::yield_frame();

    push_term("NexusOS 0.1  (RV32IM)");
    push_term("Type 'help'. Try 'open https://example.com'.");
    let n = copy_str(&mut st().url, "https://example.com");
    st().url_len = n;
    st().boot_done = true;
}

fn draw_chrome(title: &str) {
    let stt = st();
    let w = stt.fb_w;
    let h = stt.fb_h;
    s::fill(0, 0, w, h, COL_BG);
    s::fill(0, 0, w, 32, COL_TOP);
    s::text(12, 8, "NexusOS", COL_FG, 1);
    s::text(88, 8, title, COL_MUTED, 1);
    let mut clk = [0u8; 8];
    let n = clock_text(&mut clk);
    let label = core::str::from_utf8(&clk[..n]).unwrap_or("");
    s::text(w - 56, 8, label, COL_MUTED, 1);
    s::fill(0, h - 28, w, 28, COL_TOP);
    s::text(12, h - 20, "RV32IM  ·  SBI gpu  ·  HTTP bridge", COL_MUTED, 1);
}

fn draw_icon(x: i32, y: i32, label: &str, hover: bool) {
    let bg = if hover { COL_ICON_H } else { COL_ICON };
    s::fill(x, y, 148, 118, bg);
    s::fill(x + 44, y + 22, 60, 48, COL_PANEL);
    s::text(x + 18, y + 86, label, COL_FG, 1);
}

fn desktop(m: &s::Mouse, prev: u32) {
    let w = st().fb_w;
    let h = st().fb_h;
    draw_chrome("desktop");
    s::text(40, 56, "Virtual computer", COL_MUTED, 1);

    let bx = 56;
    let by = 100;
    let tx = 232;
    let ty = 100;
    let hb = m.x >= bx && m.x < bx + 148 && m.y >= by && m.y < by + 118;
    let ht = m.x >= tx && m.x < tx + 148 && m.y >= ty && m.y < ty + 118;
    draw_icon(bx, by, "Browser", hb);
    s::text(bx + 54, by + 38, "W", COL_ACCENT, 2);
    draw_icon(tx, ty, "Terminal", ht);
    s::text(tx + 54, ty + 38, ">_", COL_ACCENT, 1);

    s::text(40, h - 64, "Click an app. Esc returns here.", COL_MUTED, 1);
    let _ = w;

    if clicked(m, bx, by, 148, 118, prev) {
        st().mode = MODE_BROWSER;
        st().url_focus = true;
        set_status("Address bar focused");
    }
    if clicked(m, tx, ty, 148, 118, prev) {
        st().mode = MODE_TERM;
    }
}

fn term_history_push(line: &[u8]) {
    if line.is_empty() {
        return;
    }
    let stt = st();
    if stt.term_history_n > 0 {
        let last = stt.term_history_n - 1;
        if stt.term_history_len[last] as usize == line.len()
            && &stt.term_history[last][..line.len()] == line
        {
            stt.term_history_i = stt.term_history_n;
            return;
        }
    }
    if stt.term_history_n == TERM_HISTORY_CAP {
        for i in 0..TERM_HISTORY_CAP - 1 {
            stt.term_history[i] = stt.term_history[i + 1];
            stt.term_history_len[i] = stt.term_history_len[i + 1];
        }
        stt.term_history_n -= 1;
    }
    let i = stt.term_history_n;
    stt.term_history[i][..line.len()].copy_from_slice(line);
    stt.term_history_len[i] = line.len() as u8;
    stt.term_history_n += 1;
    stt.term_history_i = stt.term_history_n;
}

fn term_history_apply(index: usize) {
    let stt = st();
    if index >= stt.term_history_n {
        stt.term_in_len = 0;
        stt.term_cursor = 0;
        stt.term_history_i = stt.term_history_n;
        return;
    }
    let n = stt.term_history_len[index] as usize;
    stt.term_in[..n].copy_from_slice(&stt.term_history[index][..n]);
    stt.term_in_len = n;
    stt.term_cursor = n;
    stt.term_history_i = index;
}

fn term_handle_line() {
    let n = st().term_in_len;
    let mut line = [0u8; 96];
    line[..n].copy_from_slice(&st().term_in[..n]);
    st().term_in_len = 0;
    st().term_cursor = 0;
    term_history_push(&line[..n]);

    let mut shown = [0u8; 100];
    shown[0] = b'$';
    shown[1] = b' ';
    let c = n.min(94);
    shown[2..2 + c].copy_from_slice(&line[..c]);
    let sline = core::str::from_utf8(&shown[..2 + c]).unwrap_or("");
    push_term(sline);
    if n == 0 {
        return;
    }

    if eq(&line[..n], "help") {
        push_term("help: ls pwd cat head wc write touch rm clear history");
        push_term("uname ps free date whoami dmesg echo fetch open reboot");
    } else if eq(&line[..n], "uname") || eq(&line[..n], "uname -a") {
        push_term("NexusOS 0.1 nexus riscv32 RV32IM");
    } else if eq(&line[..n], "whoami") {
        push_term("guest");
    } else if eq(&line[..n], "pwd") {
        push_term("/");
    } else if eq(&line[..n], "ps") {
        push_term("PID  STATE  COMMAND");
        push_term("  1  sleep  init");
        push_term("  2  run    window-manager");
        push_term("  3  run    nexus-shell");
        push_term("  4  wait   virtual-browser");
    } else if eq(&line[..n], "history") {
        let count = st().term_history_n;
        for i in 0..count {
            let len = st().term_history_len[i] as usize;
            if let Ok(cmd) = core::str::from_utf8(&st().term_history[i][..len]) {
                push_term(cmd);
            }
        }
    } else if eq(&line[..n], "ls") {
        let mut buf = [0u8; 512];
        let k = s::fs_list(&mut buf);
        if k > 0 {
            if let Ok(t) = core::str::from_utf8(&buf[..k as usize]) {
                for part in t.split('\n') {
                    if !part.is_empty() {
                        push_term(part);
                    }
                }
            }
        } else {
            push_term("(empty)");
        }
    } else if starts(&line[..n], "cat ") || starts(&line[..n], "head ") || starts(&line[..n], "wc ") {
        let command_len = if starts(&line[..n], "cat ") { 4 } else { 5 };
        let command = core::str::from_utf8(&line[..command_len]).unwrap_or("");
        let name = core::str::from_utf8(&line[command_len..n]).unwrap_or("").trim();
        let mut buf = [0u8; 1024];
        let k = s::fs_read(name, &mut buf);
        if k < 0 {
            push_term("not found");
        } else {
            let len = (k as usize).min(buf.len());
            if command == "wc " {
                let lines = buf[..len].iter().filter(|&&b| b == b'\n').count() + usize::from(len > 0 && buf[len - 1] != b'\n');
                let mut out = [0u8; 80];
                let mut p = copy_str(&mut out, "lines=");
                p += itoa(lines as u32, &mut out[p..]);
                p += copy_str(&mut out[p..], " bytes=");
                p += itoa(len as u32, &mut out[p..]);
                if let Ok(text) = core::str::from_utf8(&out[..p]) {
                    push_term(text);
                }
            } else if let Ok(text) = core::str::from_utf8(&buf[..len]) {
                let limit = if command == "head " { 5 } else { usize::MAX };
                for part in text.split('\n').take(limit) {
                    push_term(part);
                }
            } else {
                push_term("file is not valid UTF-8 text");
            }
        }
    } else if eq(&line[..n], "dmesg") {
        let mut buf = [0u8; 1024];
        let k = s::fs_read("/var/log/dmesg", &mut buf);
        if k > 0 {
            if let Ok(t) = core::str::from_utf8(&buf[..k as usize]) {
                for part in t.split('\n') {
                    if !part.is_empty() {
                        push_term(part);
                    }
                }
            }
        }
    } else if eq(&line[..n], "free") {
        push_term("RAM  32 MiB virtual  (guest)");
        push_term("disk 8 MiB NEXDISK1");
    } else if eq(&line[..n], "date") {
        let mut b = [0u8; 16];
        let k = itoa(s::time_ms(), &mut b);
        let mut line2 = [0u8; 24];
        let p = copy_str(&mut line2, "uptime_ms ");
        line2[p..p + k].copy_from_slice(&b[..k]);
        if let Ok(t) = core::str::from_utf8(&line2[..p + k]) {
            push_term(t);
        }
    } else if starts(&line[..n], "echo ") {
        if let Ok(t) = core::str::from_utf8(&line[5..n]) {
            push_term(t);
        }
    } else if starts(&line[..n], "touch ") {
        let name = core::str::from_utf8(&line[6..n]).unwrap_or("").trim();
        if !name.is_empty() && s::fs_write(name, &[]) == 0 {
            push_term("created");
        } else {
            push_term("touch failed");
        }
    } else if starts(&line[..n], "rm ") {
        let name = core::str::from_utf8(&line[3..n]).unwrap_or("").trim();
        push_term(if s::fs_remove(name) == 0 { "removed" } else { "not found" });
    } else if starts(&line[..n], "write ") {
        let rest = &line[6..n];
        if let Some(sp) = rest.iter().position(|&c| c == b' ') {
            let name = core::str::from_utf8(&rest[..sp]).unwrap_or("");
            let data = &rest[sp + 1..];
            push_term(if s::fs_write(name, data) == 0 { "written" } else { "write failed" });
        } else {
            push_term("usage: write <file> <text>");
        }
    } else if eq(&line[..n], "clear") {
        st().term_len = 0;
    } else if eq(&line[..n], "reboot") {
        s::log("reboot requested\n");
        st().mode = MODE_DESKTOP;
    } else if starts(&line[..n], "fetch ") {
        let url = core::str::from_utf8(&line[6..n]).unwrap_or("").trim();
        push_term("GET via safety-limited HTTP bridge...");
        let rc = s::net_tx(url);
        if rc == -2 {
            push_term("NETWORK UNAVAILABLE");
        } else {
            st().loading = true;
        }
    } else if starts(&line[..n], "open ") {
        let url = core::str::from_utf8(&line[5..n]).unwrap_or("").trim();
        let stt = st();
        stt.url_len = copy_str(&mut stt.url, url);
        stt.mode = MODE_BROWSER;
        stt.url_focus = false;
        navigate(false);
    } else {
        push_term("command not found; type 'help'");
    }
}

fn terminal(m: &s::Mouse, prev: u32, key: i32) {
    let w = st().fb_w;
    let h = st().fb_h;
    draw_chrome("terminal");
    s::fill(16, 44, w - 32, h - 88, COL_TERM_BG);
    if clicked(m, w - 40, 4, 24, 24, prev) {
        st().mode = MODE_DESKTOP;
        return;
    }
    s::fill(w - 36, 6, 20, 20, COL_RED);
    s::text(w - 30, 10, "x", COL_FG, 1);

    let stt = st();
    let mut y = 56;
    for i in 0..stt.term_len {
        let n = stt.term_n[i] as usize;
        if let Ok(t) = core::str::from_utf8(&stt.term_lines[i][..n]) {
            s::text(28, y, t, COL_TERM_FG, 1);
        }
        y += 16;
    }
    let mut prompt = [0u8; 100];
    prompt[0] = b'$';
    prompt[1] = b' ';
    let n = stt.term_in_len.min(96);
    prompt[2..2 + n].copy_from_slice(&stt.term_in[..n]);
    if let Ok(t) = core::str::from_utf8(&prompt[..2 + n]) {
        s::text(28, y, t, COL_FG, 1);
    }
    if (s::time_ms() / 400) % 2 == 0 {
        s::fill(28 + (2 + stt.term_cursor as i32) * 8, y, 7, 12, COL_FG);
    }

    if key > 0 {
        let down = (key as u32) & 0x10000 != 0;
        let code = (key as u32) & 0xffff;
        if down {
            if code == KEY_ENTER {
                term_handle_line();
            } else if code == KEY_BS {
                let cursor = st().term_cursor;
                let len = st().term_in_len;
                if cursor > 0 {
                    st().term_in.copy_within(cursor..len, cursor - 1);
                    st().term_in_len -= 1;
                    st().term_cursor -= 1;
                }
            } else if code == KEY_DELETE {
                let cursor = st().term_cursor;
                let len = st().term_in_len;
                if cursor < len {
                    st().term_in.copy_within(cursor + 1..len, cursor);
                    st().term_in_len -= 1;
                }
            } else if code == KEY_LEFT {
                st().term_cursor = st().term_cursor.saturating_sub(1);
            } else if code == KEY_RIGHT {
                st().term_cursor = (st().term_cursor + 1).min(st().term_in_len);
            } else if code == KEY_HOME || code == KEY_SELECT_ALL {
                st().term_cursor = 0;
            } else if code == KEY_END {
                st().term_cursor = st().term_in_len;
            } else if code == KEY_UP {
                let index = st().term_history_i.saturating_sub(1);
                if st().term_history_n > 0 && st().term_history_i > 0 {
                    term_history_apply(index);
                }
            } else if code == KEY_DOWN {
                if st().term_history_i < st().term_history_n {
                    term_history_apply(st().term_history_i + 1);
                }
            } else if code == KEY_ESC {
                st().mode = MODE_DESKTOP;
            } else if (0x20..0x7f).contains(&code) && st().term_in_len < 95 {
                let cursor = st().term_cursor;
                let len = st().term_in_len;
                st().term_in.copy_within(cursor..len, cursor + 1);
                st().term_in[cursor] = code as u8;
                st().term_in_len += 1;
                st().term_cursor += 1;
            } else if code == KEY_TAB && st().term_in_len < 92 {
                let cursor = st().term_cursor;
                let len = st().term_in_len;
                st().term_in.copy_within(cursor..len, cursor + 4);
                st().term_in[cursor..cursor + 4].copy_from_slice(b"    ");
                st().term_in_len += 4;
                st().term_cursor += 4;
            }
        }
    }

    if st().loading {
        let mut buf = [0u8; 4096];
        let rc = s::net_rx(&mut buf);
        if rc == -1 {
            // pending
        } else if rc == -2 {
            push_term("NETWORK UNAVAILABLE");
            st().loading = false;
        } else if rc < 0 {
            push_term("fetch failed");
            st().loading = false;
        } else {
            st().loading = false;
            let n = (rc as usize).min(buf.len());
            if let Ok(t) = core::str::from_utf8(&buf[..n.min(400)]) {
                push_term("ok");
                for part in t.split('\n').take(8) {
                    push_term(part);
                }
            } else {
                push_term("ok (binary)");
            }
        }
    }
}

fn hist_push() {
    let stt = st();
    if stt.url_len == 0 {
        return;
    }
    if stt.hist_n > 0 && stt.hist_i < stt.hist_n {
        let i = stt.hist_i;
        let n = stt.hist_len[i] as usize;
        if &stt.history[i][..n] == &stt.url[..stt.url_len] {
            return;
        }
    }
    if stt.hist_n > 0 && stt.hist_i + 1 < stt.hist_n {
        stt.hist_n = stt.hist_i + 1;
    }
    if stt.hist_n < 8 {
        let i = stt.hist_n;
        stt.history[i][..stt.url_len].copy_from_slice(&stt.url[..stt.url_len]);
        stt.hist_len[i] = stt.url_len as u16;
        stt.hist_n += 1;
        stt.hist_i = i;
    } else {
        for i in 0..7 {
            stt.history[i] = stt.history[i + 1];
            stt.hist_len[i] = stt.hist_len[i + 1];
        }
        stt.history[7][..stt.url_len].copy_from_slice(&stt.url[..stt.url_len]);
        stt.hist_len[7] = stt.url_len as u16;
        stt.hist_i = 7;
    }
}

fn normalize_address() -> bool {
    let mut raw_buf = [0u8; 256];
    let raw_n = st().url_len.min(raw_buf.len());
    raw_buf[..raw_n].copy_from_slice(&st().url[..raw_n]);
    let raw = core::str::from_utf8(&raw_buf[..raw_n]).unwrap_or("").trim();
    let mut out = [0u8; 256];
    let mut n = 0usize;
    let searching = raw.chars().any(char::is_whitespace)
        || (!raw.is_empty() && !raw.contains('.') && !raw.contains('/') && !raw.contains(':'));
    if searching {
        n = copy_str(&mut out, "https://html.duckduckgo.com/html/?q=");
        for byte in raw.bytes() {
            if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
                if n < out.len() {
                    out[n] = byte;
                    n += 1;
                }
            } else if byte.is_ascii_whitespace() {
                if n < out.len() {
                    out[n] = b'+';
                    n += 1;
                }
            } else if n + 3 <= out.len() {
                const HEX: &[u8; 16] = b"0123456789ABCDEF";
                out[n] = b'%';
                out[n + 1] = HEX[(byte >> 4) as usize];
                out[n + 2] = HEX[(byte & 0x0f) as usize];
                n += 3;
            }
        }
    } else if raw.starts_with("https://") || raw.starts_with("http://") {
        n = copy_str(&mut out, raw);
    } else if !raw.is_empty() {
        n = copy_str(&mut out, "https://");
        let count = raw.len().min(out.len() - n);
        out[n..n + count].copy_from_slice(&raw.as_bytes()[..count]);
        n += count;
    }
    let stt = st();
    stt.url.fill(0);
    stt.url[..n].copy_from_slice(&out[..n]);
    stt.url_len = n;
    searching
}

fn resolve_href(current: &str, href: &str, out: &mut [u8]) -> usize {
    let href = href.trim();
    if href.is_empty() || href.starts_with('#') {
        return 0;
    }
    if href.starts_with("https://") || href.starts_with("http://") {
        return copy_str(out, href);
    }
    if href.starts_with("//") {
        let mut n = copy_str(out, "https:");
        n += copy_str(&mut out[n..], href);
        return n;
    }
    if href.contains(':') {
        return 0;
    }
    let scheme_end = match current.find("://") {
        Some(index) => index + 3,
        None => return 0,
    };
    let authority_end = current[scheme_end..]
        .find('/')
        .map(|index| scheme_end + index)
        .unwrap_or(current.len());
    let origin = &current[..authority_end];
    let mut n = copy_str(out, origin);
    if href.starts_with('/') {
        n += copy_str(&mut out[n..], href);
        return n;
    }
    let path = current[authority_end..].split(['?', '#']).next().unwrap_or("");
    let directory_end = path.rfind('/').map(|index| index + 1).unwrap_or(1);
    let directory = if path.is_empty() { "/" } else { &path[..directory_end] };
    n += copy_str(&mut out[n..], directory);
    n += copy_str(&mut out[n..], href);
    n.min(out.len())
}

fn hist_apply(i: usize) {
    let stt = st();
    if i >= stt.hist_n {
        return;
    }
    let n = stt.hist_len[i] as usize;
    stt.url[..n].copy_from_slice(&stt.history[i][..n]);
    stt.url_len = n;
    stt.hist_i = i;
    navigate(true);
}

fn navigate(from_hist: bool) {
    let searching = normalize_address();
    if !from_hist {
        hist_push();
    }
    st().page_scroll = 0;
    set_status(if searching { "Searching the web..." } else { "Fetching..." });
    st().page_len = 0;
    let url = url_ref();
    if url.is_empty() {
        set_status("Empty URL");
        return;
    }
    let rc = s::net_tx(url);
    if rc == -2 {
        set_status("NETWORK UNAVAILABLE");
        st().loading = false;
        st().page_len = copy_str(
            &mut st().page,
            "<h1>NETWORK UNAVAILABLE</h1><p>The HTTP bridge is offline or denied.</p>",
        );
    } else {
        st().loading = true;
    }
}

fn browser(m: &s::Mouse, prev: u32, key: i32) {
    let w = st().fb_w;
    let h = st().fb_h;
    draw_chrome("web");
    s::fill(w - 36, 6, 20, 20, COL_RED);
    s::text(w - 30, 10, "x", COL_FG, 1);
    if clicked(m, w - 40, 4, 24, 24, prev) {
        st().mode = MODE_DESKTOP;
        return;
    }

    // toolbar
    let bar_y = 40;
    s::fill(8, bar_y, w - 16, 32, COL_PANEL);
    let back = clicked(m, 14, bar_y + 4, 28, 24, prev);
    let fwd = clicked(m, 46, bar_y + 4, 28, 24, prev);
    let reload = clicked(m, 78, bar_y + 4, 28, 24, prev);
    s::fill(14, bar_y + 4, 28, 24, COL_BTN);
    s::text(22, bar_y + 10, "<", COL_FG, 1);
    s::fill(46, bar_y + 4, 28, 24, COL_BTN);
    s::text(54, bar_y + 10, ">", COL_FG, 1);
    s::fill(78, bar_y + 4, 28, 24, COL_BTN);
    s::text(86, bar_y + 10, "R", COL_FG, 1);

    let url_x = 114;
    let url_w = w - 200;
    s::fill(url_x, bar_y + 4, url_w, 24, COL_URL);
    if st().url_focus {
        s::fill(url_x, bar_y + 26, url_w, 1, COL_ACCENT);
    }
    s::text(url_x + 6, bar_y + 10, url_ref(), COL_FG, 1);
    s::fill(w - 78, bar_y + 4, 56, 24, COL_BTN);
    s::text(w - 62, bar_y + 10, "Go", COL_FG, 1);

    if clicked(m, url_x, bar_y + 4, url_w, 24, prev) {
        st().url_focus = true;
    }
    if clicked(m, w - 78, bar_y + 4, 56, 24, prev) {
        st().url_focus = false;
        navigate(false);
    }
    if back && st().hist_i > 0 {
        hist_apply(st().hist_i - 1);
    }
    if fwd && st().hist_i + 1 < st().hist_n {
        hist_apply(st().hist_i + 1);
    }
    if reload {
        navigate(true);
    }

    if m.wheel != 0 {
        let next = st().page_scroll + m.wheel * 80;
        st().page_scroll = next.clamp(0, 0x7fff);
    }

    let content_y = 80;
    let content_h = h - 116;
    s::fill(8, content_y, w - 16, content_h, 0xf0f0f2);

    if st().loading {
        s::text(24, content_y + 24, "Loading...", 0x333333, 1);
        s::text(24, content_y + 44, url_ref(), 0x666666, 1);
        let mut buf = [0u8; 65536];
        let rc = s::net_rx(&mut buf);
        if rc == -1 {
            // pending
        } else if rc == -2 {
            st().loading = false;
            set_status("NETWORK UNAVAILABLE");
            st().page_len = copy_str(
                &mut st().page,
                "<h1>NETWORK UNAVAILABLE</h1><p>Enable the internet bridge.</p>",
            );
        } else if rc < 0 {
            st().loading = false;
            set_status("Request failed");
            st().page_len = copy_str(
                &mut st().page,
                "<h1>Request failed</h1><p>The HTTP bridge rejected this URL.</p>",
            );
        } else {
            st().loading = false;
            let n = (rc as usize).min(st().page.len());
            st().page[..n].copy_from_slice(&buf[..n]);
            st().page_len = n;
            set_status("Loaded");
        }
    }

    if st().page_len > 0 {
        let page = &st().page[..st().page_len];
        s::html(page, 8, content_y, w - 16, content_h, st().page_scroll);
    } else if !st().loading {
        s::text(24, content_y + 24, "Enter a URL and press Go.", 0x444444, 1);
        s::text(24, content_y + 48, "Try https://example.com", 0x38488f, 1);
    }

    s::fill(8, h - 36, w - 16, 8, COL_TOP);
    s::text(16, h - 20, status_ref(), COL_MUTED, 1);
    let mut scroll_label = [0u8; 24];
    let scroll_n = copy_str(&mut scroll_label, "PAGE ");
    let number_n = itoa(st().page_scroll as u32 / 80 + 1, &mut scroll_label[scroll_n..]);
    if let Ok(label) = core::str::from_utf8(&scroll_label[..scroll_n + number_n]) {
        s::text(w - 96, h - 20, label, COL_MUTED, 1);
    }

    if !st().loading && m.buttons & 1 != 0 && prev & 1 == 0 {
        if m.y >= content_y && m.y < content_y + content_h {
            let idx = s::hit(m.x, m.y);
            if idx >= 0 {
                let mut href = [0u8; 256];
                let n = s::hit_url(idx, &mut href);
                if n > 0 {
                    let href_text = core::str::from_utf8(&href[..(n as usize).min(href.len())]).unwrap_or("");
                    let mut resolved = [0u8; 256];
                    let resolved_len = resolve_href(url_ref(), href_text, &mut resolved);
                    if resolved_len > 0 {
                        let stt = st();
                        stt.url.fill(0);
                        stt.url[..resolved_len].copy_from_slice(&resolved[..resolved_len]);
                        stt.url_len = resolved_len;
                        stt.url_focus = false;
                        navigate(false);
                    } else {
                        set_status("Unsupported link type");
                    }
                }
            } else {
                st().url_focus = false;
            }
        }
    }

    if key > 0 {
        let down = (key as u32) & 0x10000 != 0;
        let code = (key as u32) & 0xffff;
        if down {
            if code == KEY_ESC {
                st().mode = MODE_DESKTOP;
            } else if code == KEY_SELECT_ALL && st().url_focus {
                st().url_len = 0;
            } else if (code == KEY_PAGE_UP || code == KEY_UP) && !st().url_focus {
                st().page_scroll = (st().page_scroll - 96).max(0);
            } else if (code == KEY_PAGE_DOWN || code == KEY_DOWN) && !st().url_focus {
                st().page_scroll = (st().page_scroll + 96).min(0x7fff);
            } else if st().url_focus {
                if code == KEY_ENTER {
                    st().url_focus = false;
                    navigate(false);
                } else if code == KEY_BS {
                    if st().url_len > 0 {
                        st().url_len -= 1;
                    }
                } else if (0x20..0x7f).contains(&code) && st().url_len < 250 {
                    let i = st().url_len;
                    st().url[i] = code as u8;
                    st().url_len += 1;
                }
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    boot();
    loop {
        let key = s::key();
        let m = s::mouse();
        let prev = st().prev_btn;
        match st().mode {
            MODE_TERM => terminal(&m, prev, key),
            MODE_BROWSER => browser(&m, prev, key),
            _ => desktop(&m, prev),
        }
        st().prev_btn = m.buttons;
        s::present();
        s::yield_frame();
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    s::log("PANIC\n");
    loop {
        s::yield_frame();
    }
}
