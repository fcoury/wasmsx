use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use tracing_subscriber::fmt;
use wasmsx::{
    hexdump, partial_hexdump,
    slot::{RamSlot, RomSlot, SlotType},
    vdp::DisplayMode,
    Machine, Renderer, TMS9918,
};

#[cfg(test)]
#[ctor::ctor]
fn init() {
    // let filter = EnvFilter::from_default_env();
    let fmt_subscriber = fmt::Subscriber::builder()
        // .with_env_filter(filter)
        .with_max_level(tracing::Level::DEBUG)
        .with_target(false)
        .finish();
    tracing::subscriber::set_global_default(fmt_subscriber)
        .expect("Unable to set global tracing subscriber");
}

#[test]
fn test_screen1_color() {
    let mut machine = get_machine("roms/hotbit.rom");
    machine.step_for(682025);

    let vdp = machine.get_vdp();

    tracing::info!("Calc: {:#x}", (vdp.registers[3] as usize & 0x7F) * 0x040);

    assert_eq!(vdp.display_mode, DisplayMode::Graphic1);
    assert_eq!(vdp.color_table_address, 0x2000);
}

#[test]
fn test_screen1_color_table() {
    let mut vdp = get_vdp_fixture("screen1_color_table_set", "hotbit", 682025);
    set_vdp_screen1(
        &mut vdp,
        r#"
        ....|....1....|....2....|....3..





            This is an MSX Test 123

            ABCDEFGHIJKLMNOPQRSTUVWXYZ














        ....|....1....|....2....|....3..
        "#,
    );

    tracing::info!(
        "VRAM:\n\n{}",
        partial_hexdump(&vdp.vram, 0x1800, 0x1800 + 32 * 24)
    );

    tracing::info!("\n\n{}", partial_hexdump(&vdp.vram, 0x2000, 0x2020));

    let mut renderer = Renderer::new(&vdp);
    renderer.draw();
    // tracing::info!(
    //     "\n\n{}",
    //     partial_hexdump(&renderer.screen_buffer, 0, 32 * 24 * 8)
    // );
}

fn set_vdp_screen1(vdp: &mut TMS9918, text: &str) {
    let lines = text.trim().split('\n');

    // finds location of first non space character in lines[0]
    let first_char = lines
        .clone()
        .next()
        .unwrap()
        .chars()
        .position(|c| c != ' ')
        .unwrap_or(0);

    let mut pos = 0x1800;
    for line in lines {
        let s = &line.trim()[first_char..];
        for x in pos..pos + 32 {
            vdp.vram[x] = 0x20;
        }
        for (x, c) in s.chars().enumerate() {
            vdp.vram[pos + x] = c as u8;
        }
        pos += 32;
    }
}

fn get_vdp_fixture(name: &str, rom: &str, steps: usize) -> TMS9918 {
    let queue = Rc::new(RefCell::new(VecDeque::new()));
    TMS9918::new_with_vram(queue, get_vram_fixture(name, rom, steps))
}

fn get_vram_fixture(name: &str, rom: &str, steps: usize) -> Vec<u8> {
    let file_name = format!("tests/fixtures/{}-{}-{}.vram", rom, name, steps);

    if let Ok(vram) = std::fs::read(file_name.clone()) {
        return vram;
    }

    let mut machine = get_machine(&format!("roms/{}.rom", rom));
    machine.step_for(steps);

    let vdp = machine.get_vdp();
    let vram = vdp.vram;

    std::fs::write(file_name, vram).unwrap();

    vram.to_vec()
}

#[test]
fn machine_test() {
    let mut machine = get_machine("roms/hotbit.rom");

    let mut last_display_mode = None;
    loop {
        machine.step_for(10000);

        let stop = if let Some(display_mode) = last_display_mode.clone() {
            display_mode != machine.get_vdp().display_mode
        } else {
            true
        };

        if stop {
            last_display_mode = Some(machine.get_vdp().display_mode);
            tracing::error!("{:#x} Display mode: {:?}", machine.pc(), last_display_mode);
        }

        if machine.halted() {
            break;
        }
    }
}

fn get_machine(rom: &str) -> Machine {
    let rom = std::fs::read(rom).unwrap();

    let mut machine = Machine::new(&[
        SlotType::Rom(RomSlot::new(&[0; 0x8000], 0x0000, 0x8000)),
        SlotType::Empty,
        SlotType::Empty,
        SlotType::Ram(RamSlot::new(0x0000, 0x10000)),
    ]);
    machine.load_rom(0, &rom);
    machine
}
