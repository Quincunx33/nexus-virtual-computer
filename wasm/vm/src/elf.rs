//! Minimal 32-bit little-endian ELF loader (PT_LOAD only).

#[derive(Debug)]
pub enum ElfError {
    Truncated,
    Magic,
    Class,
    Endian,
    Machine,
}

pub struct LoadedElf {
    pub entry: u32,
}

pub fn load(ram: &mut [u8], ram_base: u32, elf: &[u8]) -> Result<LoadedElf, ElfError> {
    if elf.len() < 52 {
        return Err(ElfError::Truncated);
    }
    if &elf[0..4] != b"\x7fELF" {
        return Err(ElfError::Magic);
    }
    if elf[4] != 1 {
        return Err(ElfError::Class);
    }
    if elf[5] != 1 {
        return Err(ElfError::Endian);
    }
    let machine = u16::from_le_bytes([elf[18], elf[19]]);
    if machine != 0xf3 {
        // EM_RISCV
        return Err(ElfError::Machine);
    }
    let entry = u32::from_le_bytes(elf[24..28].try_into().unwrap());
    let phoff = u32::from_le_bytes(elf[28..32].try_into().unwrap()) as usize;
    let phentsize = u16::from_le_bytes([elf[42], elf[43]]) as usize;
    let phnum = u16::from_le_bytes([elf[44], elf[45]]) as usize;
    if phentsize < 32 {
        return Err(ElfError::Truncated);
    }
    for i in 0..phnum {
        let off = phoff + i * phentsize;
        if off + 32 > elf.len() {
            return Err(ElfError::Truncated);
        }
        let p_type = u32::from_le_bytes(elf[off..off + 4].try_into().unwrap());
        if p_type != 1 {
            continue; // PT_LOAD
        }
        let p_offset = u32::from_le_bytes(elf[off + 4..off + 8].try_into().unwrap()) as usize;
        let p_vaddr = u32::from_le_bytes(elf[off + 8..off + 12].try_into().unwrap());
        let p_filesz = u32::from_le_bytes(elf[off + 16..off + 20].try_into().unwrap()) as usize;
        let p_memsz = u32::from_le_bytes(elf[off + 20..off + 24].try_into().unwrap()) as usize;
        if p_vaddr < ram_base {
            continue;
        }
        let dest = (p_vaddr - ram_base) as usize;
        if dest >= ram.len() {
            continue;
        }
        let mem_end = dest.saturating_add(p_memsz).min(ram.len());
        ram[dest..mem_end].fill(0);
        if p_offset + p_filesz > elf.len() {
            return Err(ElfError::Truncated);
        }
        let copy = p_filesz.min(mem_end.saturating_sub(dest));
        ram[dest..dest + copy].copy_from_slice(&elf[p_offset..p_offset + copy]);
    }
    Ok(LoadedElf { entry })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_garbage() {
        let mut ram = vec![0u8; 1024];
        assert!(load(&mut ram, 0x8000_0000, b"not elf").is_err());
    }
}
