use std::collections::VecDeque;

/// MSX NTSC timing constants
pub const CPU_CLOCK_HZ: u32 = 3_579_545; // 3.58 MHz
pub const SCANLINES_PER_FRAME: u32 = 262;
pub const CPU_CYCLES_PER_SCANLINE: u32 = 228;
pub const ACTIVE_DISPLAY_LINES: u32 = 192;
pub const VBLANK_START_LINE: u32 = 192;
pub const FRAME_RATE: f64 = 59.94; // NTSC frame rate

/// Event types that can be scheduled
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClockEvent {
    VBlankStart,
    VBlankEnd,
    HBlankStart,
    HBlankEnd,
    ScanlineStart(u32),
    FrameEnd,
}

/// Scheduled event with timing information
#[derive(Debug)]
struct ScheduledEvent {
    cycle: u64,
    event: ClockEvent,
}

/// Master clock system for cycle-accurate emulation
pub struct Clock {
    /// Total CPU cycles executed
    total_cycles: u64,
    
    /// Current scanline (0-261)
    current_scanline: u32,
    
    /// Cycles within current scanline (0-227)
    scanline_cycle: u32,
    
    /// Frame counter
    frame_count: u64,
    
    /// Event queue
    events: VecDeque<ScheduledEvent>,
    
    /// VBlank active flag
    vblank_active: bool,
    
    /// HBlank active flag
    hblank_active: bool,
}

impl Clock {
    pub fn new() -> Self {
        let mut clock = Self {
            total_cycles: 0,
            current_scanline: 0,
            scanline_cycle: 0,
            frame_count: 0,
            events: VecDeque::new(),
            vblank_active: false,
            hblank_active: false,
        };
        
        // Schedule initial events
        clock.schedule_frame_events();
        
        clock
    }
    
    /// Reset the clock to initial state
    pub fn reset(&mut self) {
        self.total_cycles = 0;
        self.current_scanline = 0;
        self.scanline_cycle = 0;
        self.frame_count = 0;
        self.events.clear();
        self.vblank_active = false;
        self.hblank_active = false;
        
        self.schedule_frame_events();
    }
    
    /// Advance the clock by the specified number of CPU cycles
    pub fn tick(&mut self, cycles: u32) -> Vec<ClockEvent> {
        let mut triggered_events = Vec::new();
        
        for _ in 0..cycles {
            self.total_cycles += 1;
            self.scanline_cycle += 1;
            
            // Check for HBlank timing (cycles 171-227 are HBlank)
            if self.scanline_cycle == 171 && !self.hblank_active {
                self.hblank_active = true;
                triggered_events.push(ClockEvent::HBlankStart);
            }
            
            // End of scanline
            if self.scanline_cycle >= CPU_CYCLES_PER_SCANLINE {
                self.scanline_cycle = 0;
                self.hblank_active = false;
                triggered_events.push(ClockEvent::HBlankEnd);
                
                self.current_scanline += 1;
                
                // Check for VBlank start
                if self.current_scanline == VBLANK_START_LINE && !self.vblank_active {
                    self.vblank_active = true;
                    triggered_events.push(ClockEvent::VBlankStart);
                }
                
                // End of frame
                if self.current_scanline >= SCANLINES_PER_FRAME {
                    self.current_scanline = 0;
                    self.frame_count += 1;
                    
                    if self.vblank_active {
                        self.vblank_active = false;
                        triggered_events.push(ClockEvent::VBlankEnd);
                    }
                    
                    triggered_events.push(ClockEvent::FrameEnd);
                    self.schedule_frame_events();
                }
                
                triggered_events.push(ClockEvent::ScanlineStart(self.current_scanline));
            }
        }
        
        // Process scheduled events
        while let Some(event) = self.events.front() {
            if event.cycle <= self.total_cycles {
                if let Some(scheduled) = self.events.pop_front() {
                    triggered_events.push(scheduled.event);
                }
            } else {
                break;
            }
        }
        
        triggered_events
    }
    
    /// Schedule events for the current frame
    fn schedule_frame_events(&mut self) {
        // Events are now handled directly in tick() for simplicity
    }
    
    /// Get current timing information
    pub fn get_timing_info(&self) -> TimingInfo {
        TimingInfo {
            total_cycles: self.total_cycles,
            current_scanline: self.current_scanline,
            scanline_cycle: self.scanline_cycle,
            frame_count: self.frame_count,
            vblank_active: self.vblank_active,
            hblank_active: self.hblank_active,
        }
    }
    
    /// Get cycles until next frame
    pub fn cycles_until_frame_end(&self) -> u64 {
        let cycles_in_frame = self.current_scanline as u64 * CPU_CYCLES_PER_SCANLINE as u64 
                            + self.scanline_cycle as u64;
        let total_frame_cycles = SCANLINES_PER_FRAME as u64 * CPU_CYCLES_PER_SCANLINE as u64;
        total_frame_cycles - cycles_in_frame
    }
    
    /// Get progress through current frame (0.0 - 1.0)
    pub fn frame_progress(&self) -> f64 {
        let cycles_in_frame = self.current_scanline as f64 * CPU_CYCLES_PER_SCANLINE as f64 
                            + self.scanline_cycle as f64;
        let total_frame_cycles = SCANLINES_PER_FRAME as f64 * CPU_CYCLES_PER_SCANLINE as f64;
        cycles_in_frame / total_frame_cycles
    }
    
    /// Check if we're in the active display area
    pub fn is_active_display(&self) -> bool {
        self.current_scanline < ACTIVE_DISPLAY_LINES
    }
    
    /// Get current scanline
    pub fn current_scanline(&self) -> u32 {
        self.current_scanline
    }
    
    /// Get total cycles
    pub fn total_cycles(&self) -> u64 {
        self.total_cycles
    }
    
    /// Check if VBlank is active
    pub fn is_vblank(&self) -> bool {
        self.vblank_active
    }
    
    /// Check if HBlank is active
    pub fn is_hblank(&self) -> bool {
        self.hblank_active
    }
    
    /// Get frame count
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }
}

/// Timing information snapshot
#[derive(Debug, Clone)]
pub struct TimingInfo {
    pub total_cycles: u64,
    pub current_scanline: u32,
    pub scanline_cycle: u32,
    pub frame_count: u64,
    pub vblank_active: bool,
    pub hblank_active: bool,
}

impl Default for Clock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_scanline_timing() {
        let mut clock = Clock::new();
        
        // Advance one scanline
        let events = clock.tick(CPU_CYCLES_PER_SCANLINE);
        
        assert_eq!(clock.current_scanline(), 1);
        assert!(events.contains(&ClockEvent::ScanlineStart(1)));
    }
    
    #[test]
    fn test_vblank_timing() {
        let mut clock = Clock::new();
        
        // Advance to VBlank
        let cycles_to_vblank = VBLANK_START_LINE * CPU_CYCLES_PER_SCANLINE;
        let events = clock.tick(cycles_to_vblank);
        
        assert!(clock.is_vblank());
        assert!(events.contains(&ClockEvent::VBlankStart));
    }
    
    #[test]
    fn test_frame_timing() {
        let mut clock = Clock::new();
        
        // Advance one full frame
        let cycles_per_frame = SCANLINES_PER_FRAME * CPU_CYCLES_PER_SCANLINE;
        let events = clock.tick(cycles_per_frame);
        
        assert_eq!(clock.frame_count, 1);
        assert_eq!(clock.current_scanline(), 0);
        assert!(events.contains(&ClockEvent::FrameEnd));
    }
}