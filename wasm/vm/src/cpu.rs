//! RV32IM interpreter. Executes real RISC-V machine code.

#[derive(Clone, Debug)]
pub struct Cpu {
    pub x: [u32; 32],
    pub pc: u32,
    pub halted: bool,
    pub illegal: bool,
    pub cycles: u64,
    pub pending_ecall: bool,
}

impl Default for Cpu {
    fn default() -> Self {
        Self {
            x: [0; 32],
            pc: 0,
            halted: false,
            illegal: false,
            cycles: 0,
            pending_ecall: false,
        }
    }
}

impl Cpu {
    #[inline]
    pub fn get(&self, r: u32) -> u32 {
        if r == 0 { 0 } else { self.x[r as usize] }
    }

    #[inline]
    pub fn set(&mut self, r: u32, v: u32) {
        if r != 0 {
            self.x[r as usize] = v;
        }
    }
}

pub trait Bus {
    fn load8(&mut self, addr: u32) -> u8;
    fn load16(&mut self, addr: u32) -> u16;
    fn load32(&mut self, addr: u32) -> u32;
    fn store8(&mut self, addr: u32, v: u8);
    fn store16(&mut self, addr: u32, v: u16);
    fn store32(&mut self, addr: u32, v: u32);
}

#[inline]
fn sext(v: u32, bits: u32) -> u32 {
    let sh = 32 - bits;
    ((v << sh) as i32 >> sh) as u32
}

#[inline]
fn imm_i(inst: u32) -> u32 {
    sext(inst >> 20, 12)
}

#[inline]
fn imm_s(inst: u32) -> u32 {
    sext(((inst >> 25) << 5) | ((inst >> 7) & 0x1f), 12)
}

#[inline]
fn imm_b(inst: u32) -> u32 {
    let imm = ((inst >> 31) << 12)
        | (((inst >> 7) & 0x1) << 11)
        | (((inst >> 25) & 0x3f) << 5)
        | (((inst >> 8) & 0xf) << 1);
    sext(imm, 13)
}

#[inline]
fn imm_u(inst: u32) -> u32 {
    inst & 0xffff_f000
}

#[inline]
fn imm_j(inst: u32) -> u32 {
    let imm = ((inst >> 31) << 20)
        | (((inst >> 12) & 0xff) << 12)
        | (((inst >> 20) & 0x1) << 11)
        | (((inst >> 21) & 0x3ff) << 1);
    sext(imm, 21)
}

/// Step one instruction. Returns true if an `ecall` fired (SBI).
pub fn step(cpu: &mut Cpu, bus: &mut impl Bus) -> bool {
    if cpu.halted {
        return false;
    }
    cpu.pending_ecall = false;
    let pc = cpu.pc;
    let half = bus.load16(pc) as u32;
    cpu.cycles = cpu.cycles.wrapping_add(1);

    if half & 3 != 3 {
        return step_compressed(cpu, bus, pc, half as u16);
    }

    let inst = bus.load32(pc);

    let opcode = inst & 0x7f;
    let rd = (inst >> 7) & 0x1f;
    let funct3 = (inst >> 12) & 7;
    let rs1 = (inst >> 15) & 0x1f;
    let rs2 = (inst >> 20) & 0x1f;
    let funct7 = inst >> 25;

    let mut next = pc.wrapping_add(4);
    let mut ecall = false;

    match opcode {
        0x37 => {
            // LUI
            cpu.set(rd, imm_u(inst));
        }
        0x17 => {
            // AUIPC
            cpu.set(rd, pc.wrapping_add(imm_u(inst)));
        }
        0x6f => {
            // JAL
            cpu.set(rd, pc.wrapping_add(4));
            next = pc.wrapping_add(imm_j(inst));
        }
        0x67 => {
            // JALR
            let t = pc.wrapping_add(4);
            next = (cpu.get(rs1).wrapping_add(imm_i(inst))) & !1;
            cpu.set(rd, t);
        }
        0x63 => {
            let a = cpu.get(rs1);
            let b = cpu.get(rs2);
            let take = match funct3 {
                0 => a == b,                         // BEQ
                1 => a != b,                         // BNE
                4 => (a as i32) < (b as i32),        // BLT
                5 => (a as i32) >= (b as i32),       // BGE
                6 => a < b,                          // BLTU
                7 => a >= b,                         // BGEU
                _ => {
                    cpu.illegal = true;
                    false
                }
            };
            if take {
                next = pc.wrapping_add(imm_b(inst));
            }
        }
        0x03 => {
            let addr = cpu.get(rs1).wrapping_add(imm_i(inst));
            let v = match funct3 {
                0 => sext(bus.load8(addr) as u32, 8),             // LB
                1 => sext(bus.load16(addr) as u32, 16),           // LH
                2 => bus.load32(addr),                            // LW
                4 => bus.load8(addr) as u32,                      // LBU
                5 => bus.load16(addr) as u32,                     // LHU
                _ => {
                    cpu.illegal = true;
                    0
                }
            };
            cpu.set(rd, v);
        }
        0x23 => {
            let addr = cpu.get(rs1).wrapping_add(imm_s(inst));
            let v = cpu.get(rs2);
            match funct3 {
                0 => bus.store8(addr, v as u8),
                1 => bus.store16(addr, v as u16),
                2 => bus.store32(addr, v),
                _ => cpu.illegal = true,
            }
        }
        0x13 => {
            let a = cpu.get(rs1);
            let imm = imm_i(inst);
            let shamt = (inst >> 20) & 0x1f;
            let v = match funct3 {
                0 => a.wrapping_add(imm),                                // ADDI
                1 => a.wrapping_shl(shamt),                              // SLLI
                2 => ((a as i32) < (imm as i32)) as u32,                 // SLTI
                3 => (a < imm) as u32,                                   // SLTIU
                4 => a ^ imm,                                            // XORI
                5 => {
                    if funct7 & 0x20 != 0 {
                        ((a as i32) >> shamt) as u32 // SRAI
                    } else {
                        a >> shamt // SRLI
                    }
                }
                6 => a | imm,                                            // ORI
                7 => a & imm,                                            // ANDI
                _ => 0,
            };
            cpu.set(rd, v);
        }
        0x33 => {
            let a = cpu.get(rs1);
            let b = cpu.get(rs2);
            let v = if funct7 == 1 {
                // M extension
                match funct3 {
                    0 => a.wrapping_mul(b), // MUL
                    1 => ((a as i32 as i64).wrapping_mul(b as i32 as i64) >> 32) as u32, // MULH
                    2 => ((a as i32 as i64).wrapping_mul(b as i64) >> 32) as u32, // MULHSU
                    3 => ((a as u64).wrapping_mul(b as u64) >> 32) as u32, // MULHU
                    4 => {
                        // DIV
                        if b == 0 {
                            u32::MAX
                        } else if a == 0x8000_0000 && b == u32::MAX {
                            a
                        } else {
                            ((a as i32) / (b as i32)) as u32
                        }
                    }
                    5 => {
                        if b == 0 { u32::MAX } else { a / b } // DIVU
                    }
                    6 => {
                        // REM
                        if b == 0 {
                            a
                        } else if a == 0x8000_0000 && b == u32::MAX {
                            0
                        } else {
                            ((a as i32) % (b as i32)) as u32
                        }
                    }
                    7 => {
                        if b == 0 { a } else { a % b } // REMU
                    }
                    _ => 0,
                }
            } else {
                match (funct3, funct7) {
                    (0, 0x00) => a.wrapping_add(b),                      // ADD
                    (0, 0x20) => a.wrapping_sub(b),                      // SUB
                    (1, _) => a.wrapping_shl(b & 0x1f),                  // SLL
                    (2, _) => ((a as i32) < (b as i32)) as u32,          // SLT
                    (3, _) => (a < b) as u32,                            // SLTU
                    (4, _) => a ^ b,                                     // XOR
                    (5, 0x00) => a >> (b & 0x1f),                        // SRL
                    (5, 0x20) => ((a as i32) >> (b & 0x1f)) as u32,      // SRA
                    (6, _) => a | b,                                     // OR
                    (7, _) => a & b,                                     // AND
                    _ => {
                        cpu.illegal = true;
                        0
                    }
                }
            };
            cpu.set(rd, v);
        }
        0x0f => {
            // FENCE / FENCE.I — nop
        }
        0x73 => {
            if inst == 0x0000_0073 {
                // ECALL
                ecall = true;
                cpu.pending_ecall = true;
            } else if inst == 0x0010_0073 {
                // EBREAK
                cpu.halted = true;
            } else {
                // Minimal CSRs: treat as nop / rdcycle-ish
                let csr = inst >> 20;
                match (funct3, csr) {
                    (2, 0xc00) | (2, 0xb00) => cpu.set(rd, cpu.cycles as u32), // RDCYCLE / MCYCLE
                    (2, 0xc80) | (2, 0xb80) => cpu.set(rd, (cpu.cycles >> 32) as u32),
                    (2, 0xc02) => cpu.set(rd, cpu.cycles as u32), // RDINSTRET
                    (2, 0xc82) => cpu.set(rd, (cpu.cycles >> 32) as u32),
                    (1 | 2 | 3 | 5 | 6 | 7, _) => cpu.set(rd, 0),
                    _ => {}
                }
            }
        }
        _ => {
            cpu.illegal = true;
        }
    }

    if cpu.illegal {
        cpu.halted = true;
    } else {
        cpu.pc = next;
    }
    ecall
}

fn rprime(r: u16) -> u32 {
    8 + (r as u32)
}

fn step_compressed(cpu: &mut Cpu, bus: &mut impl Bus, pc: u32, inst: u16) -> bool {
    let op = inst & 3;
    let funct3 = (inst >> 13) & 7;
    let mut next = pc.wrapping_add(2);
    let mut ecall = false;

    match (op, funct3) {
        // Quadrant 0
        (0, 0) => {
            // C.ADDI4SPN
            let nzuimm = ((((inst >> 11) & 0x3) as u32) << 4)
                | ((((inst >> 7) & 0xf) as u32) << 6)
                | ((((inst >> 6) & 0x1) as u32) << 2)
                | ((((inst >> 5) & 0x1) as u32) << 3);

            let rd = rprime((inst >> 2) & 7);
            if nzuimm == 0 {
                cpu.illegal = true;
            } else {
                cpu.set(rd, cpu.get(2).wrapping_add(nzuimm));
            }
        }
        (0, 2) => {
            // C.LW
            let rd = rprime((inst >> 2) & 7);
            let rs1 = rprime((inst >> 7) & 7);
            let off = ((((inst >> 6) & 0x1) as u32) << 2)
                | ((((inst >> 10) & 0x7) as u32) << 3)
                | ((((inst >> 5) & 0x1) as u32) << 6);
            let addr = cpu.get(rs1).wrapping_add(off);
            cpu.set(rd, bus.load32(addr));
        }
        (0, 6) => {
            // C.SW
            let rs2 = rprime((inst >> 2) & 7);
            let rs1 = rprime((inst >> 7) & 7);
            let off = ((((inst >> 6) & 0x1) as u32) << 2)
                | ((((inst >> 10) & 0x7) as u32) << 3)
                | ((((inst >> 5) & 0x1) as u32) << 6);
            let addr = cpu.get(rs1).wrapping_add(off);
            bus.store32(addr, cpu.get(rs2));
        }
        // Quadrant 1
        (1, 0) => {
            // C.ADDI / C.NOP
            let rd = ((inst >> 7) & 0x1f) as u32;
            let imm = sext(((((inst >> 12) & 1) as u32) << 5) | ((inst >> 2) & 0x1f) as u32, 6);
            if rd != 0 {
                cpu.set(rd, cpu.get(rd).wrapping_add(imm));
            }
        }
        (1, 1) => {
            // C.JAL
            let imm = cj_imm(inst);
            cpu.set(1, pc.wrapping_add(2));
            next = pc.wrapping_add(imm);
        }
        (1, 2) => {
            // C.LI
            let rd = ((inst >> 7) & 0x1f) as u32;
            let imm = sext(((((inst >> 12) & 1) as u32) << 5) | ((inst >> 2) & 0x1f) as u32, 6);
            cpu.set(rd, imm);
        }
        (1, 3) => {
            let rd = ((inst >> 7) & 0x1f) as u32;
            if rd == 2 {
                // C.ADDI16SP
                let imm = sext(
                    ((((inst >> 12) & 1) as u32) << 9)
                        | ((((inst >> 3) & 3) as u32) << 7)
                        | ((((inst >> 5) & 1) as u32) << 6)
                        | ((((inst >> 2) & 1) as u32) << 5)
                        | ((((inst >> 6) & 1) as u32) << 4),
                    10,
                );
                if imm == 0 {
                    cpu.illegal = true;
                } else {
                    cpu.set(2, cpu.get(2).wrapping_add(imm));
                }
            } else {
                // C.LUI
                let imm = sext(
                    ((((inst >> 12) & 1) as u32) << 17) | (((inst >> 2) & 0x1f) as u32) << 12,
                    18,
                );
                if rd == 0 || imm == 0 {
                    cpu.illegal = true;
                } else {
                    cpu.set(rd, imm);
                }
            }
        }
        (1, 4) => {
            let rd = rprime((inst >> 7) & 7);
            let funct2 = (inst >> 10) & 3;
            match funct2 {
                0 => {
                    // C.SRLI
                    let sh = ((((inst >> 12) & 1) as u32) << 5) | ((inst >> 2) & 0x1f) as u32;
                    cpu.set(rd, cpu.get(rd) >> (sh & 31));
                }
                1 => {
                    // C.SRAI
                    let sh = ((((inst >> 12) & 1) as u32) << 5) | ((inst >> 2) & 0x1f) as u32;
                    cpu.set(rd, ((cpu.get(rd) as i32) >> (sh & 31)) as u32);
                }
                2 => {
                    // C.ANDI
                    let imm = sext(((((inst >> 12) & 1) as u32) << 5) | ((inst >> 2) & 0x1f) as u32, 6);
                    cpu.set(rd, cpu.get(rd) & imm);
                }
                3 => {
                    let rs2 = rprime((inst >> 2) & 7);
                    let a = cpu.get(rd);
                    let b = cpu.get(rs2);
                    let bit12 = (inst >> 12) & 1;
                    let f = (inst >> 5) & 3;
                    let v = match (bit12, f) {
                        (0, 0) => a.wrapping_sub(b), // C.SUB
                        (0, 1) => a ^ b,             // C.XOR
                        (0, 2) => a | b,             // C.OR
                        (0, 3) => a & b,             // C.AND
                        _ => {
                            cpu.illegal = true;
                            a
                        }
                    };
                    cpu.set(rd, v);
                }
                _ => cpu.illegal = true,
            }
        }
        (1, 5) => {
            // C.J
            next = pc.wrapping_add(cj_imm(inst));
        }
        (1, 6) => {
            // C.BEQZ
            let rs1 = rprime((inst >> 7) & 7);
            if cpu.get(rs1) == 0 {
                next = pc.wrapping_add(cb_imm(inst));
            }
        }
        (1, 7) => {
            // C.BNEZ
            let rs1 = rprime((inst >> 7) & 7);
            if cpu.get(rs1) != 0 {
                next = pc.wrapping_add(cb_imm(inst));
            }
        }
        // Quadrant 2
        (2, 0) => {
            // C.SLLI
            let rd = ((inst >> 7) & 0x1f) as u32;
            let sh = ((((inst >> 12) & 1) as u32) << 5) | ((inst >> 2) & 0x1f) as u32;
            if rd != 0 {
                cpu.set(rd, cpu.get(rd) << (sh & 31));
            }
        }
        (2, 2) => {
            // C.LWSP
            let rd = ((inst >> 7) & 0x1f) as u32;
            let off = ((((inst >> 2) & 0x3) as u32) << 6)
                | ((((inst >> 12) & 1) as u32) << 5)
                | ((((inst >> 4) & 0x7) as u32) << 2);
            let addr = cpu.get(2).wrapping_add(off);
            cpu.set(rd, bus.load32(addr));
        }
        (2, 4) => {
            let rd = ((inst >> 7) & 0x1f) as u32;
            let rs2 = ((inst >> 2) & 0x1f) as u32;
            let bit12 = (inst >> 12) & 1;
            if bit12 == 0 && rs2 == 0 {
                // C.JR
                if rd == 0 {
                    cpu.illegal = true;
                } else {
                    next = cpu.get(rd) & !1;
                }
            } else if bit12 == 0 {
                // C.MV
                cpu.set(rd, cpu.get(rs2));
            } else if rd == 0 && rs2 == 0 {
                // C.EBREAK
                cpu.halted = true;
            } else if rs2 == 0 {
                // C.JALR
                let t = pc.wrapping_add(2);
                next = cpu.get(rd) & !1;
                cpu.set(1, t);
            } else {
                // C.ADD
                cpu.set(rd, cpu.get(rd).wrapping_add(cpu.get(rs2)));
            }
        }
        (2, 6) => {
            // C.SWSP
            let rs2 = ((inst >> 2) & 0x1f) as u32;
            let off = ((((inst >> 7) & 0x3) as u32) << 6) | ((((inst >> 9) & 0xf) as u32) << 2);
            let addr = cpu.get(2).wrapping_add(off);
            bus.store32(addr, cpu.get(rs2));
        }
        _ => {
            cpu.illegal = true;
        }
    }

    if cpu.illegal {
        cpu.halted = true;
    } else if !cpu.halted {
        cpu.pc = next;
    }
    ecall
}

fn cj_imm(inst: u16) -> u32 {
    let i = inst as u32;
    let imm = (((i >> 12) & 1) << 11)
        | (((i >> 11) & 1) << 4)
        | (((i >> 9) & 3) << 8)
        | (((i >> 8) & 1) << 10)
        | (((i >> 7) & 1) << 6)
        | (((i >> 6) & 1) << 7)
        | (((i >> 3) & 7) << 1)
        | (((i >> 2) & 1) << 5);
    sext(imm, 12)
}


fn cb_imm(inst: u16) -> u32 {
    let i = inst as u32;
    let imm = (((i >> 12) & 1) << 8)
        | (((i >> 5) & 3) << 6)
        | (((i >> 2) & 1) << 5)
        | (((i >> 10) & 3) << 3)
        | (((i >> 3) & 3) << 1);
    sext(imm, 9)
}

#[cfg(test)]
pub fn encode_i(op: u32, rd: u32, f3: u32, rs1: u32, imm: i32) -> u32 {
    let imm = imm as u32;
    op | (rd << 7) | (f3 << 12) | (rs1 << 15) | (imm << 20)
}

#[cfg(test)]
pub fn encode_r(op: u32, rd: u32, f3: u32, rs1: u32, rs2: u32, f7: u32) -> u32 {
    op | (rd << 7) | (f3 << 12) | (rs1 << 15) | (rs2 << 20) | (f7 << 25)
}

#[cfg(test)]
pub fn encode_u(op: u32, rd: u32, imm: u32) -> u32 {
    op | (rd << 7) | (imm & 0xffff_f000)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Mem(Vec<u8>);
    impl Bus for Mem {
        fn load8(&mut self, a: u32) -> u8 {
            self.0.get(a as usize).copied().unwrap_or(0)
        }
        fn load16(&mut self, a: u32) -> u16 {
            u16::from_le_bytes([self.load8(a), self.load8(a + 1)])
        }
        fn load32(&mut self, a: u32) -> u32 {
            u32::from_le_bytes([
                self.load8(a),
                self.load8(a + 1),
                self.load8(a + 2),
                self.load8(a + 3),
            ])
        }
        fn store8(&mut self, a: u32, v: u8) {
            if (a as usize) < self.0.len() {
                self.0[a as usize] = v;
            }
        }
        fn store16(&mut self, a: u32, v: u16) {
            let b = v.to_le_bytes();
            self.store8(a, b[0]);
            self.store8(a + 1, b[1]);
        }
        fn store32(&mut self, a: u32, v: u32) {
            let b = v.to_le_bytes();
            for i in 0..4 {
                self.store8(a + i, b[i as usize]);
            }
        }
    }

    fn run(words: &[u32]) -> Cpu {
        let mut bytes = vec![0u8; 4096];
        for (i, w) in words.iter().enumerate() {
            bytes[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
        }
        let mut cpu = Cpu::default();
        let mut mem = Mem(bytes);
        for _ in 0..64 {
            if cpu.halted || cpu.pc as usize >= words.len() * 4 {
                break;
            }
            step(&mut cpu, &mut mem);
        }
        cpu
    }

    #[test]
    fn addi_add() {
        let cpu = run(&[
            encode_i(0x13, 1, 0, 0, 5),
            encode_i(0x13, 2, 0, 0, 7),
            encode_r(0x33, 3, 0, 1, 2, 0),
            0x0010_0073, // ebreak
        ]);
        assert_eq!(cpu.get(3), 12);
    }

    #[test]
    fn lui_addi() {
        let cpu = run(&[
            encode_u(0x37, 1, 0x1234_5000),
            encode_i(0x13, 1, 0, 1, 0x67),
            0x0010_0073,
        ]);
        assert_eq!(cpu.get(1), 0x1234_5067);
    }

    #[test]
    fn load_store() {
        let mut bytes = vec![0u8; 4096];
        let prog = [
            encode_i(0x13, 1, 0, 0, 42),
            encode_i(0x13, 2, 0, 0, 100),
            // sw x1, 0(x2)
            0x23 | (2 << 12) | (2 << 15) | (1 << 20),
            // lw x3, 0(x2)
            0x03 | (3 << 7) | (2 << 12) | (2 << 15),
            0x0010_0073,
        ];
        for (i, w) in prog.iter().enumerate() {
            bytes[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
        }
        let mut cpu = Cpu::default();
        let mut mem = Mem(bytes);
        for _ in 0..16 {
            if cpu.halted {
                break;
            }
            step(&mut cpu, &mut mem);
        }
        assert_eq!(cpu.get(3), 42);
    }

    #[test]
    fn jal_and_link() {
        // jal x1, +8  then ebreak at 4, addi x2,0,99 at 8
        let _jal = 0x6f | (1 << 7) | (8u32 << 21);
        // J imm for +8: imm[10:1] = 4. encoding: inst[30:21] = imm[10:1]
        let jal = 0x6f | (1 << 7) | (4 << 21);
        let cpu = run(&[jal, 0x0010_0073, encode_i(0x13, 2, 0, 0, 99), 0x0010_0073]);
        assert_eq!(cpu.get(1), 4);
        assert_eq!(cpu.get(2), 99);
    }

    #[test]
    fn mul() {
        let cpu = run(&[
            encode_i(0x13, 1, 0, 0, 6),
            encode_i(0x13, 2, 0, 0, 7),
            encode_r(0x33, 3, 0, 1, 2, 1),
            0x0010_0073,
        ]);
        assert_eq!(cpu.get(3), 42);
    }

    #[test]
    fn beq_taken() {
        // addi x1,0,1 ; addi x2,0,1 ; beq x1,x2,+8 ; addi x3,0,1 ; addi x3,0,7 ; ebreak
        let _beq = 0x63 | (0 << 12) | (1 << 15) | (2 << 20) | (8 << 7);
        // B imm +8: imm[4:1]=4 in inst[11:8], imm[11]=0, imm[10:5]=0, imm[12]=0
        // inst[11:8] = imm[4:1] = 4; inst[7] = imm[11] = 0
        let beq = 0x63 | (4 << 8) | (1 << 15) | (2 << 20);
        let cpu = run(&[
            encode_i(0x13, 1, 0, 0, 1),
            encode_i(0x13, 2, 0, 0, 1),
            beq,
            encode_i(0x13, 3, 0, 0, 1),
            encode_i(0x13, 3, 0, 0, 7),
            0x0010_0073,
        ]);
        assert_eq!(cpu.get(3), 7);
    }

    #[test]
    fn compressed_li() {
        // c.li x11, 6  then ebreak
        let mut bytes = vec![0u8; 64];
        bytes[0] = 0x99;
        bytes[1] = 0x45;
        bytes[2..6].copy_from_slice(&0x0010_0073u32.to_le_bytes());
        let mut cpu = Cpu::default();
        let mut mem = Mem(bytes);
        for _ in 0..8 {
            if cpu.halted {
                break;
            }
            step(&mut cpu, &mut mem);
        }
        assert!(!cpu.illegal, "illegal at pc={:08x}", cpu.pc);
        assert_eq!(cpu.get(11), 6);
    }

    #[test]
    fn x0_hardwired_zero() {
        let cpu = run(&[encode_i(0x13, 0, 0, 0, 99), 0x0010_0073]);
        assert_eq!(cpu.get(0), 0);
    }
}
