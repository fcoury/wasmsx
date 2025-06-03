# TMS9918 Sprite Implementation Plan

## Overview
This document outlines the plan for implementing sprite support in the MSX emulator's TMS9918 VDP emulation.

## Current State Analysis
- Basic `Sprite` struct exists with x, y, pattern, color, and collision fields
- VDP has an array of 8 sprites (should be 32 for TMS9918)
- Sprite-related registers are partially logged but not fully implemented
- No sprite rendering code exists
- Some sprite status flags are in place (collision, invalid)

## Implementation Plan

### 1. Fix Sprite Data Structures
- [ ] Change sprite array from 8 to 32 sprites (TMS9918 supports 32 sprites)
- [ ] Add sprite attribute table address calculation from Register 5
- [ ] Add sprite pattern table address calculation from Register 6
- [ ] Add sprite size and magnification flags from Register 1

### 2. Implement Sprite Attribute Table (SAT)
The Sprite Attribute Table contains 4 bytes per sprite:
- **Byte 0**: Y position (0xD0 = end of sprite list)
- **Byte 1**: X position  
- **Byte 2**: Pattern number
- **Byte 3**: Early clock bit (bit 7) and color (bits 0-3)

Address calculation: `(R5 & 0x7F) << 7`

### 3. Implement Sprite Pattern Table (SPT)
- 8 bytes per pattern for 8x8 sprites
- 32 bytes per pattern for 16x16 sprites
- Address calculation: `(R6 & 0x07) << 11`

### 4. Add Sprite Processing Methods
```rust
// Core sprite processing functions
fn load_sprites_from_sat(&mut self)     // Read sprite data from VRAM
fn evaluate_sprites(&mut self, line: u8) // Determine visible sprites on line
fn check_sprite_collision(&mut self)     // Detect overlapping sprites
fn render_sprites(&mut self, line: usize) // Draw sprites on scanline
```

### 5. Implement Sprite Rendering Pipeline
- [ ] Process sprites in order (0-31)
- [ ] Implement 4-sprite-per-line limit
- [ ] Set 5th sprite flag and number when limit exceeded
- [ ] Handle sprite priority (lower numbered sprites on top)
- [ ] Support both 8x8 and 16x16 sprite sizes
- [ ] Support 1x and 2x magnification

### 6. Add Sprite Status Handling
Status Register (S#0) sprite-related bits:
- **Bit 5 (F)**: Frame flag (set during vblank)
- **Bit 6 (5S)**: 5th sprite flag (more than 4 sprites on a line)
- **Bit 7 (C)**: Collision flag (two sprites overlap)
- **Bits 0-4**: 5th sprite number

Additional collision tracking:
- [ ] Store collision X/Y coordinates for debugging
- [ ] Track which sprites collided

### 7. Integration Points
- [ ] Update `render_graphic1()` and `render_graphic2()` to call sprite rendering
- [ ] Add sprite evaluation during line rendering
- [ ] Update status register reading to include sprite flags
- [ ] Clear sprite flags at appropriate times

### 8. Testing Strategy
- [ ] Create test ROM that displays sprites
- [ ] Test sprite limits (4 per line)
- [ ] Test collision detection
- [ ] Test different sprite sizes and magnification
- [ ] Verify sprite priority ordering

## Implementation Priority Order
1. Fix sprite count and basic structures
2. Implement SAT/SPT address calculations
3. Add sprite loading from VRAM
4. Implement basic 8x8 sprite rendering
5. Add sprite limits and status flags
6. Add collision detection
7. Add 16x16 sprites and magnification
8. Optimize and test thoroughly

## Additional Features from WebMSX Analysis

### Performance Optimizations
- [ ] Integrate sprite rendering directly into line rendering loop
- [ ] Pre-calculate sprite visibility per scanline
- [ ] Cache sprite pattern data when possible

### Edge Cases and Special Handling
- [ ] Handle early clock bit (EC) for sprite X coordinate
- [ ] Implement proper sprite coordinate wrapping
- [ ] Handle Y=208 (0xD0) as end-of-sprite marker
- [ ] Support transparent color 0 for sprites

### Debug Features (Optional)
- [ ] Sprite disable toggle
- [ ] Individual sprite visibility control
- [ ] Sprite bounding box display
- [ ] Collision point visualization

### Important Implementation Notes
- Sprites with Y=208 or Y=216 mark end of active sprite list
- Early Clock bit shifts sprite 32 pixels to the left
- Sprite collision only occurs between non-transparent pixels
- Color 0 is always transparent for sprites
- Sprites are drawn in order, with sprite 0 having highest priority

## Technical Details

### VDP Registers Related to Sprites
- **R1 bit 1**: Sprite size (0=8x8, 1=16x16)
- **R1 bit 0**: Sprite magnification (0=1x, 1=2x)
- **R5**: Sprite Attribute Table base address
- **R6**: Sprite Pattern Table base address
- **R7**: Border/background color (affects sprite transparency)

### Sprite Rendering Algorithm
1. For each scanline:
   - Load sprites from SAT
   - Filter sprites that appear on current line
   - Sort by sprite number (for priority)
   - Render up to 4 sprites
   - Set 5S flag if more than 4 sprites
   - Check for collisions
   - Draw sprite pixels (non-zero color pixels)

### Memory Layout
- SAT: 128 bytes (32 sprites × 4 bytes)
- SPT: 2KB for 8x8 sprites, 8KB for 16x16 sprites

## References
- TMS9918 Technical Data Manual
- MSX Technical Handbook
- [MSX.org VDP Documentation](https://www.msx.org/wiki/TMS9918)