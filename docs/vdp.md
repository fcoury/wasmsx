# 2.2.1 Register 1 (contains 8 VDP option control bits)

```
BIT 0   4/16K selection

        0 selects 4027 RAM operation
        1 selects 4108/4116 RAM operation

BIT 1   BLANK enable/disable

        0 causes the active display area to blank
        1 enables the active display
        Blanking causes the display to show border color only

BIT 2   IE (Interrupt Enable)

        0 disables VDP interrupt
        1 enables VDP interrupt

BIT 3,4 M1, M2 (mode bits 1 and 2)

        M1, M2 and M3 determine the operating mode of the VDP:


          M1    M2    M3    Mode              Note
           0     0     0    Graphics I mode   32x24 Text Mode on the MSX
           0     0     1    Graphics II mode
           0     1     0    Multicolor Mode
           1     0     0    Text mode         40x24 Text Mode on the MSX

BIT 5   Reserved

BIT 6   Size (sprite size select)

        0 selects Size 0 sprites (8 × 8 bit)
        1 selects Size 1 sprites (16 × 16 bits)

BIT 7   MAG (Magnification option for sprites)

        0 selects MAGO sprites (1X)
        1 selects MAG1 sprites (2X)
```
