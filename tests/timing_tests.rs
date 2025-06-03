#[cfg(test)]
mod timing_tests {
    use wasmsx::clock::{Clock, ClockEvent, CPU_CYCLES_PER_SCANLINE, SCANLINES_PER_FRAME};

    #[test]
    fn test_frame_timing() {
        let mut clock = Clock::new();

        // Advance exactly one frame
        let cycles_per_frame = SCANLINES_PER_FRAME * CPU_CYCLES_PER_SCANLINE;
        let events = clock.tick(cycles_per_frame);

        // Should have received VBlank start, end, and frame end events
        assert!(events.contains(&ClockEvent::VBlankStart));
        assert!(events.contains(&ClockEvent::VBlankEnd));
        assert!(events.contains(&ClockEvent::FrameEnd));

        // Should be at the start of the next frame
        assert_eq!(clock.current_scanline(), 0);
        assert_eq!(clock.frame_count(), 1);
    }

    #[test]
    fn test_vblank_timing() {
        let mut clock = Clock::new();

        // Advance to VBlank start (line 192)
        let cycles_to_vblank = 192 * CPU_CYCLES_PER_SCANLINE;
        let events = clock.tick(cycles_to_vblank);

        // Should be in VBlank
        assert!(clock.is_vblank());
        assert!(events.contains(&ClockEvent::VBlankStart));
        assert_eq!(clock.current_scanline(), 192);
    }

    #[test]
    fn test_hblank_timing() {
        let mut clock = Clock::new();

        // Advance to HBlank within first scanline
        let events = clock.tick(171);

        // Should be in HBlank
        assert!(clock.is_hblank());
        assert!(events.contains(&ClockEvent::HBlankStart));

        // Advance to end of scanline
        let events = clock.tick(57); // 228 - 171 = 57

        // Should have left HBlank
        assert!(!clock.is_hblank());
        assert!(events.contains(&ClockEvent::HBlankEnd));
        assert_eq!(clock.current_scanline(), 1);
    }

    #[test]
    fn test_frame_progress() {
        let mut clock = Clock::new();

        // At start, progress should be 0
        assert_eq!(clock.frame_progress(), 0.0);

        // Halfway through frame
        let half_frame_cycles = (SCANLINES_PER_FRAME * CPU_CYCLES_PER_SCANLINE) / 2;
        clock.tick(half_frame_cycles);

        let progress = clock.frame_progress();
        assert!(
            progress > 0.49 && progress < 0.51,
            "Progress was {}",
            progress
        );

        // Near end of frame
        let almost_full = SCANLINES_PER_FRAME * CPU_CYCLES_PER_SCANLINE - CPU_CYCLES_PER_SCANLINE;
        let mut clock2 = Clock::new();
        clock2.tick(almost_full);

        let progress2 = clock2.frame_progress();
        assert!(progress2 > 0.99, "Progress was {}", progress2);
    }
}

