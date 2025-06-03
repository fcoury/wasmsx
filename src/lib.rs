pub mod bus;
pub mod fdc;
pub mod instruction;
pub mod internal_state;
pub mod keyboard;
pub mod machine;
pub mod ppi;
pub mod psg;
pub mod renderer;
pub mod slot;
pub mod utils;
pub mod vdp;

use std::sync::Once;

pub use fdc::{DiskFormat, DiskImage, WD2793};
pub use internal_state::{InternalState, ReportState};
pub use machine::MachineBuilder;
pub use machine::{Machine, ProgramEntry};
pub use renderer::Renderer;
use tracing_wasm::WASMLayerConfigBuilder;
pub use utils::{compare_slices, hexdump, partial_hexdump};
pub use vdp::TMS9918;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;

pub fn get_machine(rom_data: &[u8]) -> Machine {
    MachineBuilder::new()
        .rom_slot(rom_data, 0x0000, 0x10000)
        .empty_slot()
        .empty_slot()
        .ram_slot(0x0000, 0x10000)
        .build()
}

pub fn get_machine_with_disk(rom_data: &[u8], disk_rom_data: &[u8]) -> Machine {
    // Determine disk ROM size and placement
    tracing::info!("Disk ROM size: {} bytes", disk_rom_data.len());
    let disk_rom_size = disk_rom_data.len() as u32;
    let (base_addr, size) = match disk_rom_size {
        0x4000 => (0x4000, 0x4000),   // 16KB disk ROM at 0x4000-0x7FFF
        0x8000 => (0x4000, 0x8000),   // 32KB disk ROM at 0x4000-0xBFFF
        0x10000 => (0x0000, 0x10000), // 64KB disk ROM fills entire slot
        _ => {
            // For non-standard sizes, try to fit at 0x4000
            if disk_rom_size <= 0x4000 {
                (0x4000, disk_rom_size)
            } else if disk_rom_size <= 0xC000 {
                (0x4000, disk_rom_size)
            } else {
                (0x0000, disk_rom_size.min(0x10000))
            }
        }
    };

    tracing::info!(
        "Disk ROM base address: 0x{:04X}, size: {} bytes",
        base_addr,
        size
    );
    
    // Check disk ROM header
    if disk_rom_data.len() >= 2 {
        tracing::info!(
            "Disk ROM header: {:02X} {:02X} (should be 41 42 for 'AB' or similar)",
            disk_rom_data[0],
            disk_rom_data[1]
        );
    }
    
    MachineBuilder::new()
        .rom_slot(rom_data, 0x0000, 0x10000) // Slot 0: Main BIOS
        .rom_slot(disk_rom_data, base_addr as u16, size) // Slot 1: Disk ROM
        .empty_slot() // Slot 2: Empty
        .ram_slot(0x0000, 0x10000) // Slot 3: RAM
        .with_disk_support() // Enable FDC
        .build()
}

static INIT: Once = Once::new();

fn init_tracing() {
    INIT.call_once(|| {
        tracing_wasm::set_as_global_default_with_config(
            WASMLayerConfigBuilder::default()
                .set_max_level(tracing::Level::DEBUG)
                .build(),
        );
    });
}

#[wasm_bindgen(js_name = Machine)]
pub struct JsMachine(Machine);

#[wasm_bindgen(js_class = Machine)]
impl JsMachine {
    #[wasm_bindgen(constructor)]
    pub fn new(rom_data: &[u8]) -> Self {
        console_error_panic_hook::set_once();
        init_tracing();
        // tracing_wasm::set_as_global_default();

        Self(get_machine(rom_data))
    }

    #[wasm_bindgen(js_name = newWithDisk)]
    pub fn new_with_disk(rom_data: &[u8], disk_rom_data: &[u8]) -> Result<JsMachine, JsValue> {
        console_error_panic_hook::set_once();

        init_tracing();
        tracing::info!("Creating machine with disk support");

        // Validate ROM sizes
        if rom_data.is_empty() {
            return Err(JsValue::from_str("BIOS ROM data is empty"));
        }

        if disk_rom_data.is_empty() {
            return Err(JsValue::from_str("Disk ROM data is empty"));
        }

        if disk_rom_data.len() > 0x10000 {
            return Err(JsValue::from_str(&format!(
                "Disk ROM too large: {} bytes (max 65536)",
                disk_rom_data.len()
            )));
        }

        Ok(Self(get_machine_with_disk(rom_data, disk_rom_data)))
    }

    #[wasm_bindgen(getter)]
    pub fn pc(&self) -> u16 {
        self.0.pc()
    }

    #[wasm_bindgen(getter)]
    pub fn ram(&self) -> Vec<u8> {
        self.0.ram()
    }

    pub fn step_for(&mut self, n: usize) {
        self.0.step_for(n);
    }

    pub fn screen(&self) -> Vec<u8> {
        let mut bus = self.0.bus.borrow_mut();
        bus.vdp.pulse();
        let mut renderer = Renderer::new(&bus.vdp);
        renderer.draw();
        renderer.screen_buffer.to_vec()
    }

    #[wasm_bindgen(getter)]
    pub fn vram(&self) -> Vec<u8> {
        self.0.bus.borrow().vdp.vram.to_vec()
    }

    #[wasm_bindgen(getter = displayMode)]
    pub fn display_mode(&self) -> String {
        format!("{:?}", self.0.bus.borrow().vdp.display_mode)
    }

    #[wasm_bindgen(js_name=keyDown)]
    pub fn key_down(&mut self, key: String) {
        self.0.bus.borrow_mut().key_down(key);
    }

    #[wasm_bindgen(js_name=keyUp)]
    pub fn key_up(&mut self, key: String) {
        self.0.bus.borrow_mut().key_up(key);
    }

    #[wasm_bindgen(js_name=enableDiskSystem)]
    pub fn enable_disk_system(&mut self) {
        self.0.enable_disk_system();
    }

    #[wasm_bindgen(js_name=insertDisk)]
    pub fn insert_disk(&mut self, drive: usize, data: &[u8], filename: &str) {
        let disk_image = DiskImage::from_file(data.to_vec(), filename);
        self.0.insert_disk(drive, disk_image);
    }

    #[wasm_bindgen(js_name=ejectDisk)]
    pub fn eject_disk(&mut self, drive: usize) {
        self.0.eject_disk(drive);
    }
}
