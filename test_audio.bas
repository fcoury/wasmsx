10 REM Simple PSG test - Play tones on all 3 channels
20 REM Channel A - 440Hz (A4 note)
30 OUT &HA0,0:OUT &HA1,254
40 OUT &HA0,1:OUT &HA1,0
50 REM Channel B - 523Hz (C5 note)  
60 OUT &HA0,2:OUT &HA1,214
70 OUT &HA0,3:OUT &HA1,0
80 REM Channel C - 659Hz (E5 note)
90 OUT &HA0,4:OUT &HA1,170
100 OUT &HA0,5:OUT &HA1,0
110 REM Noise period
120 OUT &HA0,6:OUT &HA1,16
130 REM Mixer - Enable all tone channels, disable noise
140 OUT &HA0,7:OUT &HA1,&HF8
150 REM Set volumes for all channels
160 OUT &HA0,8:OUT &HA1,15
170 OUT &HA0,9:OUT &HA1,15
180 OUT &HA0,10:OUT &HA1,15
190 REM Wait for keypress
200 PRINT "Playing A major chord (A4, C5, E5)"
210 PRINT "Press any key to stop"
220 A$=INKEY$:IF A$="" THEN 220
230 REM Turn off all channels
240 OUT &HA0,8:OUT &HA1,0
250 OUT &HA0,9:OUT &HA1,0
260 OUT &HA0,10:OUT &HA1,0
270 PRINT "Audio stopped"