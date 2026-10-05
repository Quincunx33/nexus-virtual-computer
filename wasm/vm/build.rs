use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let generated = manifest.join("../target/riscv32imac-unknown-none-elf/release/nexus-os");
    let bundled = manifest.join("../guest/nexus-os.elf");
    let source = env::var_os("NEXUS_GUEST_ELF")
        .map(PathBuf::from)
        .unwrap_or_else(|| if generated.exists() { generated.clone() } else { bundled.clone() });
    let output = PathBuf::from(env::var("OUT_DIR").unwrap()).join("guest.elf");
    println!("cargo:rerun-if-changed={}", generated.display());
    println!("cargo:rerun-if-changed={}", bundled.display());
    println!("cargo:rerun-if-env-changed=NEXUS_GUEST_ELF");
    if source.exists() {
        fs::copy(&source, &output).expect("copy guest ELF");
        println!("cargo:warning=embedded guest ELF from {}", source.display());
    } else {
        fs::write(&output, []).expect("placeholder guest ELF");
        println!("cargo:warning=NexusOS ELF missing at {} — build the guest first", source.display());
    }
}
