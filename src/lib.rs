pub mod bus;
pub mod clock;
pub mod cpu_extensions;
pub mod disk_drive;
pub mod disk_driver;
pub mod disk_error;
pub mod disk_rom_manager;
pub mod dsk_image;
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

pub use internal_state::{InternalState, ReportState};
use js_sys::Float32Array;
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

pub fn get_machine_with_rom(bios_rom_data: &[u8], slot1_rom_data: &[u8]) -> Machine {
    // Determine disk ROM size and placement
    tracing::info!("ROM size: {} bytes", slot1_rom_data.len());
    let rom_size = slot1_rom_data.len() as u32;
    let (base_addr, size) = match rom_size {
        0x4000 => (0x4000, 0x4000),   // 16KB disk ROM at 0x4000-0x7FFF
        0x8000 => (0x4000, 0x8000),   // 32KB disk ROM at 0x4000-0xBFFF
        0x10000 => (0x0000, 0x10000), // 64KB disk ROM fills entire slot
        _ => {
            // For non-standard sizes, try to fit at 0x4000
            if rom_size <= 0x4000 {
                (0x4000, rom_size)
            } else if rom_size <= 0xC000 {
                (0x4000, rom_size)
            } else {
                (0x0000, rom_size.min(0x10000))
            }
        }
    };

    tracing::info!(
        "Disk ROM base address: 0x{:04X}, size: {} bytes",
        base_addr,
        size
    );

    // Check disk ROM header
    if slot1_rom_data.len() >= 2 {
        tracing::info!(
            "Disk ROM header: {:02X} {:02X} (should be 41 42 for 'AB' or similar)",
            slot1_rom_data[0],
            slot1_rom_data[1]
        );
    }

    MachineBuilder::new()
        .rom_slot(bios_rom_data, 0x0000, 0x10000) // Slot 0: Main BIOS
        .rom_slot(slot1_rom_data, base_addr as u16, size) // Slot 1: Disk ROM
        .empty_slot() // Slot 2: Empty
        .ram_slot(0x0000, 0x10000) // Slot 3: RAM
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

    #[wasm_bindgen(js_name = newWithRom)]
    pub fn new_with_rom(bios_rom_data: &[u8], slot1_rom_data: &[u8]) -> Result<JsMachine, JsValue> {
        console_error_panic_hook::set_once();

        init_tracing();
        tracing::info!("Creating machine with disk support");

        // Validate ROM sizes
        if bios_rom_data.is_empty() {
            return Err(JsValue::from_str("BIOS ROM data is empty"));
        }

        if slot1_rom_data.is_empty() {
            return Err(JsValue::from_str("Slot 1 ROM data is empty"));
        }

        if slot1_rom_data.len() > 0x10000 {
            return Err(JsValue::from_str(&format!(
                "Disk ROM too large: {} bytes (max 65536)",
                slot1_rom_data.len()
            )));
        }

        Ok(Self(get_machine_with_rom(bios_rom_data, slot1_rom_data)))
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

    #[wasm_bindgen(js_name = stepFrame)]
    pub fn step_frame(&mut self) {
        self.0.step_frame();
    }

    #[wasm_bindgen(js_name = isFrameReady)]
    pub fn is_frame_ready(&self) -> bool {
        self.0.is_frame_ready()
    }

    #[wasm_bindgen(js_name = getFrameProgress)]
    pub fn get_frame_progress(&self) -> f64 {
        self.0.get_frame_progress()
    }

    pub fn screen(&self) -> Vec<u8> {
        let mut bus = self.0.bus.borrow_mut();
        bus.vdp.pulse();
        // Don't evaluate sprites here - it should be done once per frame during vblank
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

    #[wasm_bindgen(js_name=generateAudioSamples)]
    pub fn generate_audio_samples(&mut self, sample_count: usize) -> Float32Array {
        let mut samples = Vec::with_capacity(sample_count);
        let mut bus = self.0.bus.borrow_mut();

        // If we don't have enough samples, run the emulation to generate more
        while !bus.psg.has_samples(sample_count) {
            // Release the borrow before stepping the machine
            drop(bus);
            // Step the machine for a small number of cycles to generate more samples
            self.0.step_for(1000);
            bus = self.0.bus.borrow_mut();
        }

        // Collect samples from the PSG buffer
        for _ in 0..sample_count {
            samples.push(bus.psg.get_audio_sample());
        }

        // Convert to JavaScript Float32Array
        Float32Array::from(&samples[..])
    }
    
    #[wasm_bindgen(js_name=hasDiskSystem)]
    pub fn has_disk_system(&self) -> bool {
        self.0.has_disk_system()
    }
    
    #[wasm_bindgen(js_name=insertDisk)]
    pub fn insert_disk(&mut self, drive: u8, data: &[u8], _filename: &str) -> Result<(), JsValue> {
        self.0.load_disk_image(drive, data.to_vec())
            .map_err(|e| JsValue::from_str(&e))
    }
    
    #[wasm_bindgen(js_name=loadDiskImage)]
    pub fn load_disk_image(&mut self, drive: u8, data: &[u8]) -> Result<(), JsValue> {
        self.0.load_disk_image(drive, data.to_vec())
            .map_err(|e| JsValue::from_str(&e))
    }
    
    #[wasm_bindgen(js_name=ejectDisk)]
    pub fn eject_disk(&mut self, drive: u8) -> Result<(), JsValue> {
        self.0.eject_disk(drive)
            .map_err(|e| JsValue::from_str(&e))
    }
    
    #[wasm_bindgen(js_name=enableDiskSystem)]
    pub fn enable_disk_system(&mut self) -> Result<(), JsValue> {
        // Disk system is automatically enabled when a disk ROM is detected
        // This method exists for compatibility with the frontend
        if self.0.has_disk_system() {
            Ok(())
        } else {
            Err(JsValue::from_str("Disk system not available. Load a disk ROM in slot 1."))
        }
    }
}
