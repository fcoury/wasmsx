use tracing_subscriber::fmt;
use wasmsx::{
    slot::{RamSlot, RomSlot, SlotType},
    Machine,
};

#[test]
fn machine_test() {
    // let filter = EnvFilter::from_default_env();
    let fmt_subscriber = fmt::Subscriber::builder()
        // .with_env_filter(filter)
        .with_max_level(tracing::Level::DEBUG)
        .with_target(false)
        .finish();
    tracing::subscriber::set_global_default(fmt_subscriber)
        .expect("Unable to set global tracing subscriber");

    let mut machine = Machine::new(&[
        SlotType::Rom(RomSlot::new(&[0; 0x8000], 0x0000, 0x8000)),
        SlotType::Empty,
        SlotType::Empty,
        SlotType::Ram(RamSlot::new(0x0000, 0x10000)),
    ]);
    // read the binary file roms/hotbit.rom
    let rom = std::fs::read("roms/hotbit.rom").unwrap();

    machine.load_rom(0, &rom);

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
