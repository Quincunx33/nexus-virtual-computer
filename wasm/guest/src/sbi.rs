#![allow(dead_code)]

#[inline(always)]
pub fn ecall(nr: i32, a0: i32, a1: i32, a2: i32, a3: i32, a4: i32, a5: i32) -> i32 {
    let mut ret = a0;
    unsafe {
        core::arch::asm!(
            "ecall",
            inout("a0") ret,
            in("a1") a1,
            in("a2") a2,
            in("a3") a3,
            in("a4") a4,
            in("a5") a5,
            in("a7") nr,
            options(nostack)

        );
    }
    ret
}

pub const LOG: i32 = 1;
pub const FILL: i32 = 2;
pub const TEXT: i32 = 3;
pub const PRESENT: i32 = 4;
pub const YIELD: i32 = 5;
pub const KEY: i32 = 6;
pub const MOUSE: i32 = 7;
pub const NET_TX: i32 = 8;
pub const NET_RX: i32 = 9;
pub const HTML: i32 = 10;
pub const HIT: i32 = 11;
pub const HIT_URL: i32 = 12;
pub const FS: i32 = 13;
pub const TIME: i32 = 14;
pub const FBINFO: i32 = 15;
pub const HALT: i32 = 16;

#[repr(C)]
pub struct Mouse {
    pub x: i32,
    pub y: i32,
    pub buttons: u32,
    pub wheel: i32,
}

pub fn log(s: &str) {
    ecall(LOG, s.as_ptr() as i32, s.len() as i32, 0, 0, 0, 0);
}

pub fn fill(x: i32, y: i32, w: i32, h: i32, color: u32) {
    ecall(FILL, x, y, w, h, color as i32, 0);
}

pub fn text(x: i32, y: i32, s: &str, color: u32, scale: i32) {
    ecall(TEXT, x, y, s.as_ptr() as i32, s.len() as i32, color as i32, scale);
}

pub fn present() {
    ecall(PRESENT, 0, 0, 0, 0, 0, 0);
}

pub fn yield_frame() {
    ecall(YIELD, 0, 0, 0, 0, 0, 0);
}

pub fn key() -> i32 {
    ecall(KEY, 0, 0, 0, 0, 0, 0)
}

pub fn mouse() -> Mouse {
    let mut m = Mouse { x: 0, y: 0, buttons: 0, wheel: 0 };
    ecall(MOUSE, &mut m as *mut Mouse as i32, 0, 0, 0, 0, 0);
    m
}

pub fn net_tx(url: &str) -> i32 {
    ecall(NET_TX, url.as_ptr() as i32, url.len() as i32, 0, 0, 0, 0)
}

pub fn net_rx(buf: &mut [u8]) -> i32 {
    ecall(NET_RX, buf.as_mut_ptr() as i32, buf.len() as i32, 0, 0, 0, 0)
}

pub fn html(buf: &[u8], x: i32, y: i32, w: i32, h: i32, scroll_y: i32) -> i32 {
    // The legacy SBI has six argument registers. Pack a bounded scroll offset
    // beside the viewport height so existing device plumbing stays compact.
    let packed_height = (((scroll_y.max(0) as u32).min(0x7fff)) << 16) | ((h as u32) & 0xffff);
    ecall(HTML, buf.as_ptr() as i32, buf.len() as i32, x, y, w, packed_height as i32)
}

pub fn hit(x: i32, y: i32) -> i32 {
    ecall(HIT, x, y, 0, 0, 0, 0)
}

pub fn hit_url(idx: i32, buf: &mut [u8]) -> i32 {
    ecall(HIT_URL, idx, buf.as_mut_ptr() as i32, buf.len() as i32, 0, 0, 0)
}

pub fn time_ms() -> u32 {
    ecall(TIME, 0, 0, 0, 0, 0, 0) as u32
}

pub fn fb_size() -> (i32, i32) {
    let w: i32;
    let h: i32;
    unsafe {
        core::arch::asm!(
            "li a7, 15",
            "ecall",
            out("a0") w,
            out("a1") h,
            out("a7") _,
            options(nostack)
        );
    }
    (w, h)
}


pub fn fs_list(buf: &mut [u8]) -> i32 {
    ecall(FS, 0, buf.as_mut_ptr() as i32, buf.len() as i32, 0, 0, 0)
}

pub fn fs_read(name: &str, buf: &mut [u8]) -> i32 {
    ecall(
        FS,
        1,
        name.as_ptr() as i32,
        name.len() as i32,
        buf.as_mut_ptr() as i32,
        buf.len() as i32,
        0,
    )
}

pub fn fs_write(name: &str, data: &[u8]) -> i32 {
    ecall(
        FS,
        2,
        name.as_ptr() as i32,
        name.len() as i32,
        data.as_ptr() as i32,
        data.len() as i32,
        0,
    )
}

pub fn fs_remove(name: &str) -> i32 {
    ecall(FS, 3, name.as_ptr() as i32, name.len() as i32, 0, 0, 0)
}
