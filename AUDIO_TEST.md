# MSX Audio Testing

## Sound Support

The emulator now includes AY-3-8910 PSG (Programmable Sound Generator) emulation with Web Audio API integration.

## Testing Audio

1. Start the emulator:
   ```bash
   cargo make dev
   ```

2. Click the "Enable" button in the Audio section of the controls panel

3. Test audio with direct PSG commands:
   ```basic
   OUT &HA0,7:OUT &HA1,&HFE
   OUT &HA0,0:OUT &HA1,254
   OUT &HA0,1:OUT &HA1,0
   OUT &HA0,8:OUT &HA1,15
   ```
   
   This should play a continuous tone. To stop:
   ```basic
   OUT &HA0,8:OUT &HA1,0
   ```

4. For a simple BASIC program:
   ```basic
   10 REM Simple tone test
   20 OUT &HA0,0:OUT &HA1,254
   30 OUT &HA0,1:OUT &HA1,0
   40 OUT &HA0,7:OUT &HA1,&HFE
   50 OUT &HA0,8:OUT &HA1,15
   60 REM Press any key to stop
   70 A$=INKEY$:IF A$="" THEN 70
   80 OUT &HA0,8:OUT &HA1,0
   ```

4. Run the program with `RUN`

## PSG Register Reference

- Registers 0-1: Channel A tone period (12-bit)
- Registers 2-3: Channel B tone period (12-bit)
- Registers 4-5: Channel C tone period (12-bit)
- Register 6: Noise period (5-bit)
- Register 7: Mixer control (bit 0-2: tone enable, bit 3-5: noise enable)
- Register 8: Channel A volume (4-bit + envelope enable)
- Register 9: Channel B volume (4-bit + envelope enable)
- Register 10: Channel C volume (4-bit + envelope enable)
- Registers 11-12: Envelope period (16-bit)
- Register 13: Envelope shape

## Note Frequencies

Common musical note frequencies and their PSG period values:
- A4 (440Hz): Period = 254
- C5 (523Hz): Period = 214
- E5 (659Hz): Period = 170

Period calculation: Period = 111861 / frequency

## PLAY Command

The MSX BASIC PLAY command requires additional BIOS hooks and interrupt handling that may not be fully implemented yet. For now, use direct PSG register writes as shown above to test audio functionality.