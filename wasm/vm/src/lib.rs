//! NEXUS virtual machine: RV32IM CPU + devices, compiled to WebAssembly.

mod cpu;
mod elf;
mod font;
mod html;

use cpu::{step, Bus, Cpu};
use html::HitRect;

const RAM_BASE: u32 = 0x8000_0000;
const FB_BASE: u32 = 0x4000_0000;
const UART_BASE: u32 = 0x1000_0000;
const KBD_BASE: u32 = 0x1000_1000;
const MOUSE_BASE: u32 = 0x1000_2000;
const SYSCON_BASE: u32 = 0x1000_3000;
const TIME_BASE: u32 = 0x1000_4000;

const SBI_LOG: i32 = 1;
const SBI_FILL: i32 = 2;
const SBI_TEXT: i32 = 3;
const SBI_PRESENT: i32 = 4;
const SBI_YIELD: i32 = 5;
const SBI_KEY: i32 = 6;
const SBI_MOUSE: i32 = 7;
const SBI_NET_TX: i32 = 8;
const SBI_NET_RX: i32 = 9;
const SBI_HTML: i32 = 10;
const SBI_HIT: i32 = 11;
const SBI_HIT_URL: i32 = 12;
const SBI_FS: i32 = 13;
const SBI_TIME: i32 = 14;
const SBI_FBINFO: i32 = 15;
const SBI_HALT: i32 = 16;

const ST_HALT: u32 = 1;
const ST_YIELD: u32 = 2;
const ST_NEED_NET: u32 = 4;
const ST_ERROR: u32 = 8;
const ST_FB_DIRTY: u32 = 16;

const GUEST_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/guest.elf"));

// Touch keyboards and paste can deliver an entire command in one browser event.
// Keep enough room for a full URL plus paired key-down/key-up events.
const KBD_CAP: usize = 1024;
const LOG_CAP: usize = 32 * 1024;
const NET_TX_CAP: usize = 2048;
const NET_RX_CAP: usize = 1536 * 1024;
const DISK_CAP: usize = 8 * 1024 * 1024;

struct FileRec {
    name: String,
    data: Vec<u8>,
}

pub struct Vm {
    cpu: Cpu,
    ram: Vec<u8>,
    fb: Vec<u8>,
    fb_w: u32,
    fb_h: u32,
    fb_dirty: bool,
    yielded: bool,
    time_ms: u32,
    kbd: [u32; KBD_CAP],
    kbd_r: usize,
    kbd_w: usize,
    mouse_x: i32,
    mouse_y: i32,
    mouse_btn: u32,
    mouse_wheel: i32,
    log: Vec<u8>,
    net_tx: Vec<u8>,
    net_rx: Vec<u8>,
    net_pending: bool,
    net_ready: bool,
    net_status: i32,
    net_enabled: bool,
    hits: Vec<HitRect>,
    files: Vec<FileRec>,
    disk: Vec<u8>,
    disk_dirty: bool,
    halt_req: bool,
}

impl Vm {
    pub fn new(ram_mb: u32, fb_w: u32, fb_h: u32) -> Self {
        let ram_mb = ram_mb.clamp(8, 64);
        let fb_w = fb_w.clamp(320, 1920);
        let fb_h = fb_h.clamp(240, 1200);
        let mut vm = Self {
            cpu: Cpu::default(),
            ram: vec![0; (ram_mb as usize) * 1024 * 1024],
            fb: vec![0; (fb_w as usize) * (fb_h as usize) * 4],
            fb_w,
            fb_h,
            fb_dirty: true,
            yielded: false,
            time_ms: 0,
            kbd: [0; KBD_CAP],
            kbd_r: 0,
            kbd_w: 0,
            mouse_x: 0,
            mouse_y: 0,
            mouse_btn: 0,
            mouse_wheel: 0,
            log: Vec::with_capacity(LOG_CAP),
            net_tx: Vec::new(),
            net_rx: vec![0; NET_RX_CAP],
            net_pending: false,
            net_ready: false,
            net_status: 0,
            net_enabled: true,
            hits: Vec::new(),
            files: seed_files(),
            disk: vec![0; DISK_CAP],
            disk_dirty: true,
            halt_req: false,
        };
        vm.pack_disk();
        vm
    }

    pub fn boot(&mut self) -> Result<(), String> {
        if GUEST_ELF.is_empty() {
            return Err("guest ELF missing".into());
        }
        let loaded = elf::load(&mut self.ram, RAM_BASE, GUEST_ELF).map_err(|e| format!("{e:?}"))?;
        self.cpu = Cpu::default();
        self.cpu.pc = loaded.entry;
        self.log_str("host: ELF loaded, hart reset\n");
        Ok(())
    }

    fn log_str(&mut self, s: &str) {
        let b = s.as_bytes();
        if self.log.len() + b.len() > LOG_CAP {
            let drop = (self.log.len() + b.len()) - LOG_CAP;
            self.log.drain(..drop.min(self.log.len()));
        }
        self.log.extend_from_slice(b);
    }

    fn guest_slice(&self, ptr: u32, len: u32) -> Option<&[u8]> {
        if ptr < RAM_BASE {
            return None;
        }
        let off = (ptr - RAM_BASE) as usize;
        let end = off.checked_add(len as usize)?;
        self.ram.get(off..end)
    }

    fn guest_slice_mut(&mut self, ptr: u32, len: u32) -> Option<&mut [u8]> {
        if ptr < RAM_BASE {
            return None;
        }
        let off = (ptr - RAM_BASE) as usize;
        let end = off.checked_add(len as usize)?;
        self.ram.get_mut(off..end)
    }

    fn arg(&self, i: u32) -> u32 {
        self.cpu.get(10 + i) // a0+
    }

    fn set_ret(&mut self, v: i32) {
        self.cpu.set(10, v as u32);
    }

    fn sbi(&mut self) {
        let nr = self.cpu.get(17) as i32; // a7
        match nr {
            SBI_LOG => {
                let ptr = self.arg(0);
                let len = self.arg(1).min(4096);
                if let Some(s) = self.guest_slice(ptr, len) {
                    let owned = s.to_vec();
                    if let Ok(t) = std::str::from_utf8(&owned) {
                        self.log_str(t);
                    }
                }
                self.set_ret(0);
            }
            SBI_FILL => {
                let x = self.arg(0) as i32;
                let y = self.arg(1) as i32;
                let w = self.arg(2) as i32;
                let h = self.arg(3) as i32;
                let color = self.arg(4);
                self.fill(x, y, w, h, color);
                self.set_ret(0);
            }
            SBI_TEXT => {
                let x = self.arg(0) as i32;
                let y = self.arg(1) as i32;
                let ptr = self.arg(2);
                let len = self.arg(3).min(1024);
                let color = self.arg(4);
                let scale = (self.arg(5) as i32).clamp(1, 6);
                if let Some(s) = self.guest_slice(ptr, len) {
                    let owned = s.to_vec();
                    let c = [
                        ((color >> 16) & 0xff) as u8,
                        ((color >> 8) & 0xff) as u8,
                        (color & 0xff) as u8,
                        0xff,
                    ];
                    font::blit_text(
                        &mut self.fb,
                        self.fb_w as i32,
                        self.fb_w as i32,
                        self.fb_h as i32,
                        x,
                        y,
                        &owned,
                        c,
                        scale,
                    );
                    self.fb_dirty = true;
                }
                self.set_ret(0);
            }
            SBI_PRESENT => {
                self.fb_dirty = true;
                self.set_ret(0);
            }
            SBI_YIELD => {
                self.yielded = true;
                self.set_ret(0);
            }
            SBI_KEY => {
                if self.kbd_r == self.kbd_w {
                    self.set_ret(0);
                } else {
                    let v = self.kbd[self.kbd_r] as i32;
                    self.kbd_r = (self.kbd_r + 1) % KBD_CAP;
                    self.set_ret(v);
                }
            }
            SBI_MOUSE => {
                let ptr = self.arg(0);
                let mx = self.mouse_x as u32;
                let my = self.mouse_y as u32;
                let mb = self.mouse_btn;
                let mw = self.mouse_wheel as u32;
                self.mouse_wheel = 0;
                if let Some(dst) = self.guest_slice_mut(ptr, 16) {
                    dst[0..4].copy_from_slice(&mx.to_le_bytes());
                    dst[4..8].copy_from_slice(&my.to_le_bytes());
                    dst[8..12].copy_from_slice(&mb.to_le_bytes());
                    dst[12..16].copy_from_slice(&mw.to_le_bytes());
                    self.set_ret(0);
                } else {
                    self.set_ret(-1);
                }
            }
            SBI_NET_TX => {
                let ptr = self.arg(0);
                let len = self.arg(1).min(NET_TX_CAP as u32);
                if !self.net_enabled {
                    self.set_ret(-2);
                    return;
                }
                if let Some(s) = self.guest_slice(ptr, len) {
                    self.net_tx = s.to_vec();
                    self.net_pending = true;
                    self.net_ready = false;
                    self.set_ret(0);
                } else {
                    self.set_ret(-1);
                }
            }
            SBI_NET_RX => {
                if !self.net_enabled {
                    self.set_ret(-2);
                    return;
                }
                if self.net_pending && !self.net_ready {
                    self.set_ret(-1); // pending
                    return;
                }
                if !self.net_ready {
                    self.set_ret(-3); // none
                    return;
                }
                let ptr = self.arg(0);
                let cap = self.arg(1);
                let n = (self.net_status.max(0) as u32).min(cap);
                let data = self.net_rx[..n as usize].to_vec();
                if let Some(dst) = self.guest_slice_mut(ptr, n) {
                    dst.copy_from_slice(&data);
                    self.net_ready = false;
                    self.set_ret(self.net_status);
                } else {
                    self.set_ret(-1);
                }
            }
            SBI_HTML => {
                let ptr = self.arg(0);
                let len = self.arg(1).min(NET_RX_CAP as u32);
                let x = self.arg(2) as i32;
                let y = self.arg(3) as i32;
                let w = self.arg(4) as i32;
                let packed_height = self.arg(5);
                let h = (packed_height & 0xffff) as i32;
                let scroll_y = (packed_height >> 16) as i32;
                if let Some(s) = self.guest_slice(ptr, len) {
                    let owned = s.to_vec();
                    self.hits = html::render(
                        &owned,
                        &mut self.fb,
                        self.fb_w as i32,
                        self.fb_h as i32,
                        x,
                        y,
                        w,
                        h,
                        scroll_y,
                    );
                    self.fb_dirty = true;
                    self.set_ret(self.hits.len() as i32);
                } else {
                    self.set_ret(-1);
                }
            }
            SBI_HIT => {
                let x = self.arg(0) as i32;
                let y = self.arg(1) as i32;
                let mut found = -1i32;
                for (i, h) in self.hits.iter().enumerate() {
                    if x >= h.x && x < h.x + h.w && y >= h.y && y < h.y + h.h {
                        found = i as i32;
                        break;
                    }
                }
                self.set_ret(found);
            }
            SBI_HIT_URL => {
                let idx = self.arg(0) as usize;
                let ptr = self.arg(1);
                let cap = self.arg(2) as usize;
                let href = self.hits.get(idx).map(|h| h.href.clone());
                if let Some(h) = href {
                    let b = h.as_bytes();
                    let n = b.len().min(cap);
                    if let Some(dst) = self.guest_slice_mut(ptr, n as u32) {
                        dst.copy_from_slice(&b[..n]);
                        self.set_ret(n as i32);
                    } else {
                        self.set_ret(-1);
                    }
                } else {
                    self.set_ret(-1);
                }
            }
            SBI_FS => {
                let op = self.arg(0);
                match op {
                    0 => {
                        // list into buffer as \n-separated names
                        let ptr = self.arg(1);
                        let cap = self.arg(2) as usize;
                        let mut blob = String::new();
                        for f in &self.files {
                            blob.push_str(&f.name);
                            blob.push('\n');
                        }
                        let n = blob.len().min(cap);
                        if let Some(dst) = self.guest_slice_mut(ptr, n as u32) {
                            dst.copy_from_slice(&blob.as_bytes()[..n]);
                            self.set_ret(n as i32);
                        } else {
                            self.set_ret(-1);
                        }
                    }
                    1 => {
                        // read
                        let nptr = self.arg(1);
                        let nlen = self.arg(2);
                        let dptr = self.arg(3);
                        let dcap = self.arg(4) as usize;
                        let name = self
                            .guest_slice(nptr, nlen)
                            .and_then(|s| std::str::from_utf8(s).ok())
                            .unwrap_or("")
                            .to_string();
                        if let Some(f) = self.files.iter().find(|f| f.name == name) {
                            let n = f.data.len().min(dcap);
                            let data = f.data[..n].to_vec();
                            if let Some(dst) = self.guest_slice_mut(dptr, n as u32) {
                                dst.copy_from_slice(&data);
                                self.set_ret(n as i32);
                            } else {
                                self.set_ret(-1);
                            }
                        } else {
                            self.set_ret(-4);
                        }
                    }
                    2 => {
                        // write
                        let nptr = self.arg(1);
                        let nlen = self.arg(2);
                        let dptr = self.arg(3);
                        let dlen = self.arg(4) as usize;
                        let name = self
                            .guest_slice(nptr, nlen)
                            .and_then(|s| std::str::from_utf8(s).ok())
                            .unwrap_or("")
                            .to_string();
                        if name.is_empty() {
                            self.set_ret(-1);
                            return;
                        }
                        let data = self
                            .guest_slice(dptr, dlen as u32)
                            .map(|s| s.to_vec())
                            .unwrap_or_default();
                        if let Some(f) = self.files.iter_mut().find(|f| f.name == name) {
                            f.data = data;
                        } else {
                            self.files.push(FileRec { name, data });
                        }
                        self.pack_disk();
                        self.set_ret(0);
                    }
                    3 => {
                        let nptr = self.arg(1);
                        let nlen = self.arg(2);
                        let name = self
                            .guest_slice(nptr, nlen)
                            .and_then(|s| std::str::from_utf8(s).ok())
                            .unwrap_or("")
                            .to_string();
                        let before = self.files.len();
                        self.files.retain(|f| f.name != name);
                        self.pack_disk();
                        self.set_ret(if self.files.len() < before { 0 } else { -4 });
                    }
                    _ => self.set_ret(-1),
                }
            }
            SBI_TIME => {
                self.set_ret(self.time_ms as i32);
            }
            SBI_FBINFO => {
                self.cpu.set(10, self.fb_w);
                self.cpu.set(11, self.fb_h);
            }
            SBI_HALT => {
                self.cpu.halted = true;
                self.set_ret(0);
            }
            _ => self.set_ret(-99),
        }
    }

    fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, color: u32) {
        let c = [
            ((color >> 16) & 0xff) as u8,
            ((color >> 8) & 0xff) as u8,
            (color & 0xff) as u8,
            0xff,
        ];
        let fw = self.fb_w as i32;
        let fh = self.fb_h as i32;
        let x1 = x.max(0);
        let y1 = y.max(0);
        let x2 = (x + w).min(fw);
        let y2 = (y + h).min(fh);
        for py in y1..y2 {
            for px in x1..x2 {
                let i = ((py * fw + px) * 4) as usize;
                if i + 3 < self.fb.len() {
                    self.fb[i] = c[0];
                    self.fb[i + 1] = c[1];
                    self.fb[i + 2] = c[2];
                    self.fb[i + 3] = c[3];
                }
            }
        }
        self.fb_dirty = true;
    }

    fn pack_disk(&mut self) {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"NEXDISK1");
        buf.extend_from_slice(&(self.files.len() as u32).to_le_bytes());
        for f in &self.files {
            let nb = f.name.as_bytes();
            buf.extend_from_slice(&(nb.len() as u16).to_le_bytes());
            buf.extend_from_slice(nb);
            buf.extend_from_slice(&(f.data.len() as u32).to_le_bytes());
            buf.extend_from_slice(&f.data);
        }
        self.disk.fill(0);
        let n = buf.len().min(self.disk.len());
        self.disk[..n].copy_from_slice(&buf[..n]);
        self.disk_dirty = true;
    }

    pub fn unpack_disk(&mut self) -> bool {
        if self.disk.len() < 12 || &self.disk[..8] != b"NEXDISK1" {
            return false;
        }
        let n = u32::from_le_bytes(self.disk[8..12].try_into().unwrap()) as usize;
        let mut off = 12usize;
        let mut files = Vec::new();
        for _ in 0..n.min(256) {
            if off + 2 > self.disk.len() {
                break;
            }
            let nl = u16::from_le_bytes(self.disk[off..off + 2].try_into().unwrap()) as usize;
            off += 2;
            if off + nl + 4 > self.disk.len() {
                break;
            }
            let name = String::from_utf8_lossy(&self.disk[off..off + nl]).to_string();
            off += nl;
            let dl = u32::from_le_bytes(self.disk[off..off + 4].try_into().unwrap()) as usize;
            off += 4;
            if off + dl > self.disk.len() {
                break;
            }
            files.push(FileRec {
                name,
                data: self.disk[off..off + dl].to_vec(),
            });
            off += dl;
        }
        if !files.is_empty() {
            self.files = files;
            true
        } else {
            false
        }
    }

    pub fn run(&mut self, max_ops: u32) -> u32 {
        self.yielded = false;
        for _ in 0..max_ops {
            if self.cpu.halted {
                break;
            }
            if self.net_pending && !self.net_ready {
                break;
            }
            let mut cpu = core::mem::take(&mut self.cpu);
            let ecall = step(&mut cpu, self);
            self.cpu = cpu;
            if self.halt_req {
                self.cpu.halted = true;
            }
            if ecall {
                self.sbi();
            }
            if self.yielded || self.cpu.halted || self.cpu.illegal {
                break;
            }
        }
        let mut st = 0u32;
        if self.cpu.halted {
            st |= ST_HALT;
        }
        if self.yielded {
            st |= ST_YIELD;
        }
        if self.net_pending && !self.net_ready {
            st |= ST_NEED_NET;
        }
        if self.cpu.illegal {
            st |= ST_ERROR;
        }
        if self.fb_dirty {
            st |= ST_FB_DIRTY;
        }
        st
    }

    fn ram_load(&self, addr: u32) -> Option<usize> {
        if addr >= RAM_BASE {
            let off = (addr - RAM_BASE) as usize;
            if off < self.ram.len() {
                return Some(off);
            }
        }
        None
    }
}

impl Bus for Vm {
    fn load8(&mut self, addr: u32) -> u8 {
        if let Some(off) = self.ram_load(addr) {
            return self.ram[off];
        }
        if addr >= FB_BASE {
            let off = (addr - FB_BASE) as usize;
            if off < self.fb.len() {
                return self.fb[off];
            }
        }
        match addr {
            UART_BASE => 0,
            a if a == UART_BASE + 5 => 0x60, // LSR: THRE | TEMT
            a if a == TIME_BASE => self.time_ms as u8,
            a if (MOUSE_BASE..MOUSE_BASE + 16).contains(&a) => {
                let rel = (a - MOUSE_BASE) as usize;
                let words = [
                    self.mouse_x as u32,
                    self.mouse_y as u32,
                    self.mouse_btn,
                    self.mouse_wheel as u32,
                ];
                words[rel / 4].to_le_bytes()[rel % 4]
            }
            _ => 0,
        }
    }

    fn load16(&mut self, addr: u32) -> u16 {
        u16::from_le_bytes([self.load8(addr), self.load8(addr.wrapping_add(1))])
    }

    fn load32(&mut self, addr: u32) -> u32 {
        u32::from_le_bytes([
            self.load8(addr),
            self.load8(addr.wrapping_add(1)),
            self.load8(addr.wrapping_add(2)),
            self.load8(addr.wrapping_add(3)),
        ])
    }

    fn store8(&mut self, addr: u32, v: u8) {
        if let Some(off) = self.ram_load(addr) {
            self.ram[off] = v;
            return;
        }
        if addr >= FB_BASE {
            let off = (addr - FB_BASE) as usize;
            if off < self.fb.len() {
                self.fb[off] = v;
                self.fb_dirty = true;
            }
            return;
        }
        if addr == UART_BASE {
            self.log.push(v);
            return;
        }
        if addr == SYSCON_BASE && v == 0x55 {
            self.halt_req = true;
        }
    }

    fn store16(&mut self, addr: u32, v: u16) {
        let b = v.to_le_bytes();
        self.store8(addr, b[0]);
        self.store8(addr.wrapping_add(1), b[1]);
    }

    fn store32(&mut self, addr: u32, v: u32) {
        let b = v.to_le_bytes();
        for i in 0..4u32 {
            self.store8(addr.wrapping_add(i), b[i as usize]);
        }
    }
}

fn seed_files() -> Vec<FileRec> {
    vec![
        FileRec {
            name: "/readme.txt".into(),
            data: b"NexusOS on NEXUS virtual computer.\nType 'help' in the terminal.\nOpen the browser and try https://example.com\n".to_vec(),
        },
        FileRec {
            name: "/etc/motd".into(),
            data: b"Welcome to NexusOS.\nRV32IM hart, virtio-html browser, HTTP bridge.\n".to_vec(),
        },
        FileRec {
            name: "/var/log/dmesg".into(),
            data: b"[0.000] nexus-vm: RV32IM online\n[0.001] ram: mapped at 0x80000000\n[0.002] gpu: sbi fill/text/html\n[0.003] nic: http bridge\n".to_vec(),
        },
    ]
}

static mut VM: Option<Vm> = None;

fn vm() -> &'static mut Vm {
    unsafe { VM.as_mut().expect("vm") }
}

#[no_mangle]
pub extern "C" fn vm_create(ram_mb: u32, fb_w: u32, fb_h: u32) -> i32 {
    unsafe {
        VM = Some(Vm::new(ram_mb, fb_w, fb_h));
    }
    0
}

#[no_mangle]
pub extern "C" fn vm_boot() -> i32 {
    match vm().boot() {
        Ok(()) => 0,
        Err(e) => {
            vm().log_str(&format!("boot error: {e}\n"));
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn vm_run(max_ops: u32) -> u32 {
    vm().run(max_ops)
}

#[no_mangle]
pub extern "C" fn vm_fb_ptr() -> u32 {
    vm().fb.as_ptr() as u32
}

#[no_mangle]
pub extern "C" fn vm_fb_width() -> u32 {
    vm().fb_w
}

#[no_mangle]
pub extern "C" fn vm_fb_height() -> u32 {
    vm().fb_h
}

#[no_mangle]
pub extern "C" fn vm_fb_take_dirty() -> i32 {
    let v = vm();
    let d = v.fb_dirty as i32;
    v.fb_dirty = false;
    d
}

#[no_mangle]
pub extern "C" fn vm_key(code: u32, down: i32) {
    let v = vm();
    let packed = code | if down != 0 { 0x10000 } else { 0 };
    let next = (v.kbd_w + 1) % KBD_CAP;
    if next != v.kbd_r {
        v.kbd[v.kbd_w] = packed;
        v.kbd_w = next;
    }
}

#[no_mangle]
pub extern "C" fn vm_mouse(x: i32, y: i32, buttons: u32, wheel: i32) {
    let v = vm();
    v.mouse_x = x;
    v.mouse_y = y;
    v.mouse_btn = buttons;
    v.mouse_wheel += wheel;
}

#[no_mangle]
pub extern "C" fn vm_net_tx_ptr() -> u32 {
    vm().net_tx.as_ptr() as u32
}

#[no_mangle]
pub extern "C" fn vm_net_tx_len() -> u32 {
    vm().net_tx.len() as u32
}

#[no_mangle]
pub extern "C" fn vm_net_take_pending() -> i32 {
    vm().net_pending as i32
}

#[no_mangle]
pub extern "C" fn vm_net_rx_ptr() -> u32 {
    vm().net_rx.as_ptr() as u32
}

#[no_mangle]
pub extern "C" fn vm_net_rx_cap() -> u32 {
    vm().net_rx.len() as u32
}

#[no_mangle]
pub extern "C" fn vm_net_complete(status: i32, len: u32) {
    let v = vm();
    v.net_status = if status < 0 { status } else { len as i32 };
    if status < 0 {
        v.net_status = status;
    }
    v.net_pending = false;
    v.net_ready = true;
}

#[no_mangle]
pub extern "C" fn vm_log_ptr() -> u32 {
    vm().log.as_ptr() as u32
}

#[no_mangle]
pub extern "C" fn vm_log_len() -> u32 {
    vm().log.len() as u32
}

#[no_mangle]
pub extern "C" fn vm_log_clear() {
    vm().log.clear();
}

#[no_mangle]
pub extern "C" fn vm_instr_lo() -> u32 {
    vm().cpu.cycles as u32
}

#[no_mangle]
pub extern "C" fn vm_instr_hi() -> u32 {
    (vm().cpu.cycles >> 32) as u32
}

#[no_mangle]
pub extern "C" fn vm_pc() -> u32 {
    vm().cpu.pc
}

#[no_mangle]
pub extern "C" fn vm_disk_ptr() -> u32 {
    vm().disk.as_ptr() as u32
}

#[no_mangle]
pub extern "C" fn vm_disk_len() -> u32 {
    vm().disk.len() as u32
}

#[no_mangle]
pub extern "C" fn vm_disk_take_dirty() -> i32 {
    let v = vm();
    let d = v.disk_dirty as i32;
    v.disk_dirty = false;
    d
}

#[no_mangle]
pub extern "C" fn vm_disk_loaded() -> i32 {
    vm().unpack_disk() as i32
}

#[no_mangle]
pub extern "C" fn vm_set_time(ms: u32) {
    vm().time_ms = ms;
}

#[no_mangle]
pub extern "C" fn vm_set_net_enabled(en: i32) {
    vm().net_enabled = en != 0;
}

#[no_mangle]
pub extern "C" fn vm_halted() -> i32 {
    vm().cpu.halted as i32
}

#[no_mangle]
pub extern "C" fn vm_illegal() -> i32 {
    vm().cpu.illegal as i32
}

#[no_mangle]
pub extern "C" fn vm_ram_len() -> u32 {
    vm().ram.len() as u32
}

#[no_mangle]
pub extern "C" fn vm_xreg(i: u32) -> u32 {
    vm().cpu.get(i)
}
