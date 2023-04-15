pub mod bus;
pub mod instruction;
pub mod internal_state;
pub mod machine;
pub mod memory;
pub mod ppi;
pub mod renderer;
pub mod slot;
pub mod sound;
pub mod utils;
pub mod vdp;

pub use internal_state::{InternalState, ReportState};
use machine::MachineBuilder;
pub use machine::{Machine, ProgramEntry};
pub use renderer::Renderer;
pub use utils::compare_slices;
pub use vdp::TMS9918;
use wasm_bindgen::prelude::*;

pub fn get_machine(rom_data: &[u8]) -> Machine {
    MachineBuilder::new()
        .rom_slot(rom_data, 0x0000, 0x10000)
        .empty_slot()
        .empty_slot()
        .ram_slot(0x0000, 0x10000)
        .build()
}

#[wasm_bindgen(js_name = Machine)]
pub struct JsMachine(Machine);

#[wasm_bindgen(js_class = Machine)]
impl JsMachine {
    #[wasm_bindgen(constructor)]
    pub fn new(rom_data: &[u8]) -> Self {
        // tracing_wasm::set_as_global_default_with_config(
        //     WASMLayerConfigBuilder::default()
        //         .set_max_level(tracing::Level::DEBUG)
        //         .build(),
        // );
        tracing_wasm::set_as_global_default();

        Self(get_machine(rom_data))
    }

    #[wasm_bindgen(getter)]
    pub fn pc(&self) -> u16 {
        self.0.pc()
    }

    #[wasm_bindgen(getter)]
    pub fn ram(&self) -> Vec<u8> {
        self.0.ram()
    }

    pub fn step(&mut self) {
        self.0.step();
    }

    pub fn step_for(&mut self, n: usize) {
        for _ in 0..n {
            self.0.step();
        }
    }

    pub fn screen(&self) -> Vec<u8> {
        let cpu = self.0.cpu();
        let bus = cpu.io.bus.borrow();
        let mut renderer = Renderer::new(bus.vdp());
        renderer.draw();
        renderer.screen_buffer.to_vec()
    }

    #[wasm_bindgen(getter)]
    pub fn vram(&self) -> Vec<u8> {
        let cpu = self.0.cpu();
        let bus = cpu.io.bus.borrow();
        bus.vdp().vram.to_vec()
    }

    #[wasm_bindgen(getter)]
    pub fn text(&self) -> String {
        let cpu = self.0.cpu();
        let bus = cpu.io.bus.borrow();
        let mut renderer = Renderer::new(bus.vdp());
        renderer.as_text()
    }

    #[wasm_bindgen(getter=displayMode)]
    pub fn display_mode(&self) -> String {
        format!("{:?}", self.0.cpu().io.bus.borrow().vdp().display_mode)
    }
}
