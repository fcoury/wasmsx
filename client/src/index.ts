/**
 * This file contains the implementation of the screen rendering and user interaction
 * components of an MSX1 emulator.
 *
 * The following classes are defined in this file:
 * - Renderer: Handles the rendering of the emulator screen and data
 * - App: Manages the interaction between the emulator and the Renderer
 * - Emulator: Responsible for running the emulator logic and maintaining its state
 *
 * The main function initializes the emulator and starts the rendering loop.
 */

import init, { Machine } from "../pkg/wasmsx.js";
import { getBiosById, hasCustomRom, setCustomRom } from "./bios.js";
import { SystemManager } from "./diskrom.js";

const PROCESSOR_RATE = 3.579545 * 1000 * 1000; // MSX CPU clock
const AUDIO_SAMPLE_RATE = 44100; // Web Audio standard rate
const AUDIO_BUFFER_SIZE = 2048; // Larger buffer for stability
const PSG_NATIVE_RATE = 111860; // PSG native rate (CPU clock / 32)

const PALETTE = [
  0x000000,
  0x010101,
  0x3eb849,
  0x74d07d,
  0x5955e0,
  0x8076f1,
  0xb95e51,
  0x65dbef,
  0xdb6559,
  0xff897d,
  0xccc35e,
  0xded087,
  0x3aa241,
  0xb766b5,
  0xcccccc,
  0xffffff,
];

class Renderer {
  private screen: HTMLCanvasElement;

  private ctx: CanvasRenderingContext2D;
  private screenImageData: ImageData;

  /**
   * Constructs a new Renderer instance.
   *
   * @param {Object} screen - The dimensions of the screen
   * @param {number} screen.width - The width of the screen
   * @param {number} screen.height - The height of the screen
   */
  constructor(screen: { width: number; height: number }) {
    this.screen = document.getElementById("screen") as HTMLCanvasElement;

    const ctx = this.screen.getContext("2d");
    if (!ctx) {
      throw new Error("Could not get canvas context");
    }

    this.screen.width = screen.width;
    this.screen.height = screen.height;

    this.ctx = ctx;
    this.screenImageData = this.ctx.createImageData(
      screen.width,
      screen.height,
    );
  }

  /**
   * Renders the emulator screen.
   *
   * @param {Uint8Array} buffer - The screen buffer data
   */
  public renderScreen(buffer: Uint8Array) {
    const pixels = this.screenImageData.data;
    for (let y = 0; y < 192; y++) {
      for (let x = 0; x < 256; x++) {
        const colorOffset = y * 256 + x;
        const color = buffer[colorOffset] ?? 0;
        // Remove the skip for color 0 - we need to render black pixels too!
        const colorBytes = new Uint8Array(4);
        const paletteColor = PALETTE[color] ?? 0x000000;

        colorBytes[0] = (paletteColor >> 16) & 0xff;
        colorBytes[1] = (paletteColor >> 8) & 0xff;
        colorBytes[2] = paletteColor & 0xff;
        colorBytes[3] = 255;
        pixels.set(colorBytes, colorOffset * 4);
      }
    }

    this.ctx.putImageData(this.screenImageData, 0, 0);
  }
}

class App {
  private renderer: Renderer;
  private emulator: Emulator;

  private debugVisible: boolean;

  /**
   * Constructs a new App instance.
   * @param {Renderer} renderer - The Renderer instance
   * @param {Emulator} emulator - The Emulator instance
   */
  constructor(renderer: Renderer, emulator: Emulator) {
    this.renderer = renderer;
    this.emulator = emulator;
    this.debugVisible = false;
  }

  /**
   * Handles keyDown events.
   * @param {string} key - The key code
   * @returns {boolean} Whether the key was handled
   */
  public keyDown(key: string): boolean {
    console.log("client keyDown", key);
    return this.emulator.keyDown(key);
  }

  /**
   * Handles keyUp events.
   * @param {string} key - The key code
   * @returns {boolean} Whether the key was handled
   */
  public keyUp(key: string): boolean {
    console.log("client keyUp", key);
    const handled = this.emulator.keyUp(key);
    return handled;
  }

  /**
   * Handles a single frame of the emulator.
   * @param {number} dt - The delta time since the last frame
   */
  public frame(dt: number) {
    // Cap delta time to prevent spiral of death
    const cappedDt = Math.min(dt, 0.1); // Cap at 100ms (10 FPS minimum)

    // Add a frame limiter to prevent the emulator from running too fast
    if (cappedDt > 1 / 60) {
      this.emulator.run(1 / 60);
    } else {
      this.emulator.run(cappedDt);
    }

    // Always render for now to debug the issue
    this.renderer.renderScreen(this.emulator.getScreen());
    if (this.debugVisible) {
      this.emulator.renderState();
      this.emulator.renderVRAM();
    }
  }
}

class Emulator {
  public machine: Machine;
  private running: boolean;
  private vram: HTMLDivElement;
  private state: HTMLDivElement;
  private frameTime: number;
  private frameAccumulator: number;
  private timeBudget: number;
  private audioContext: AudioContext | null;
  private audioProcessor: ScriptProcessorNode | null;
  private audioEnabled: boolean;
  private audioSampleAccumulator: number;

  /**
   * Constructs a new Emulator instance.
   * @param {Machine} machine - The Machine instance from the wasm module
   */
  constructor(machine: Machine) {
    this.running = true;
    this.machine = machine;
    this.frameTime = 1 / 60; // Target 60 FPS
    this.frameAccumulator = 0;
    this.timeBudget = 0;
    this.vram = document.getElementById("vram") as HTMLDivElement;
    this.state = document.getElementById("state") as HTMLDivElement;
    this.audioContext = null;
    this.audioProcessor = null;
    this.audioEnabled = true;
    this.audioSampleAccumulator = 0;

    // Initialize audio on first user interaction
    const initAudio = () => {
      if (!this.audioContext) {
        this.initAudio();
        document.removeEventListener("click", initAudio);
        document.removeEventListener("keydown", initAudio);
      }
    };
    document.addEventListener("click", initAudio);
    document.addEventListener("keydown", initAudio);
  }

  /**
   * Runs the emulator for a specified duration.
   * @param {number} dt - The delta time since the last run
   */
  public run(dt: number) {
    // Use cycle-based approach with proper timing
    this.timeBudget += dt;
    const cycles = Math.floor(this.timeBudget * PROCESSOR_RATE);
    const cycleTime = cycles / PROCESSOR_RATE;
    this.timeBudget -= cycleTime;

    // Step the machine for the calculated cycles
    if (cycles > 0) {
      this.machine.step_for(cycles);
    }
  }

  /**
   * Toggles the emulator running state between running and paused.
   */
  public toggleRunning() {
    this.running = !this.running;
  }

  /**
   * Returns the emulator running state.
   * @returns {boolean} The emulator running state
   */
  public isRunning(): boolean {
    return this.running;
  }

  /**
   * Returns the screen buffer data.
   * @returns {Uint8Array} The screen buffer data
   */
  public getScreen(): Uint8Array {
    return this.machine.screen();
  }

  /**
   * Returns whether a complete frame is ready to render.
   * @returns {boolean} Whether a frame is ready
   */
  public isFrameReady(): boolean {
    return this.machine.isFrameReady();
  }

  /**
   * Renders div with PC and display mode.
   */
  public renderState() {
    const { pc, displayMode } = this.machine;

    this.state.innerHTML = `
      <div class="stateitem">
        <div class="stateitem--name">PC</div>
        <div class="stateitem--value">${pc.toString(16).padStart(4, "0")}</div>
      </div>
      <div class="stateitem">
        <div class="stateitem--name">Display Mode</div>
        <div class="stateitem--value">${displayMode}</div>
      </div>
    `;
  }

  /**
   * Renders the VRAM hex dump.
   */
  public renderVRAM() {
    const { vram } = this.machine;

    // hex dump of the vram
    const rows = [];
    for (let i = 0; i < vram.length; i += 16) {
      const row = [];
      const chars = [];
      for (let j = 0; j < 16; j++) {
        const value = vram[i + j] as number;
        row.push(value.toString(16).padStart(2, "0"));
        chars.push(
          value >= 32 && value <= 126 ? String.fromCharCode(value) : ".",
        );
      }
      const addr = i.toString(16).padStart(4, "0");
      rows.push(addr + ":  " + row.join(" ") + "  " + chars.join(""));
    }

    this.vram.innerHTML = `<pre>${
      rows
        .map((row) => `<div>${row}</div>`)
        .join("")
    }</pre>`;
  }

  /**
   * Handles keyDown events.
   * @param {string} key - The key code
   * @returns {boolean} Whether the key was handled
   */
  public keyDown(key: string): boolean {
    // console.log("key", key);
    return false;
  }

  /**
   * Handles keyUp events.
   * @param {string} key - The key code
   * @returns {boolean} Whether the key was handled
   */
  public keyUp(key: string): boolean {
    if (key === "Escape") {
      this.toggleRunning();
      return true;
    }

    return false;
  }

  /**
   * Initializes the Web Audio API for sound output.
   */
  private initAudio() {
    try {
      this.audioContext = new AudioContext({
        sampleRate: AUDIO_SAMPLE_RATE,
        latencyHint: "interactive", // Lower latency for better responsiveness
      });

      // Create a script processor for audio generation
      this.audioProcessor = this.audioContext.createScriptProcessor(
        AUDIO_BUFFER_SIZE,
        0, // no input channels
        1, // mono output
      );

      // Connect to speakers
      this.audioProcessor.connect(this.audioContext.destination);
      this.audioEnabled = true;

      console.log(
        `Audio initialized: ${AUDIO_SAMPLE_RATE}Hz, buffer size: ${AUDIO_BUFFER_SIZE}`,
      );

      this.audioProcessor.onaudioprocess = (event) => {
        const output = event.outputBuffer.getChannelData(0);
        const bufferSize = event.outputBuffer.length;

        if (!this.running || !this.audioEnabled) {
          // Fill with silence when paused or disabled
          output.fill(0);
          return;
        }

        // Calculate the number of samples to generate from the PSG
        const resampleRatio = PSG_NATIVE_RATE / AUDIO_SAMPLE_RATE;
        const samplesToGenerate = Math.ceil(bufferSize * resampleRatio);

        // Generate audio samples from the PSG
        const samples = this.machine.generateAudioSamples(samplesToGenerate);

        // Downsample from 112kHz to 44.1kHz
        let sampleIndex = 0;
        for (let i = 0; i < bufferSize; i++) {
          let total = 0;
          let count = 0;
          while (sampleIndex < (i + 1) * resampleRatio) {
            total += samples[sampleIndex] || 0;
            count++;
            sampleIndex++;
          }
          output[i] = count > 0 ? total / count : 0;
        }
      };
    } catch (error) {
      console.error("Failed to initialize audio:", error);
      this.audioEnabled = false;
    }
  }

  /**
   * Toggles audio on/off.
   */
  public toggleAudio() {
    this.audioEnabled = !this.audioEnabled;
    console.log("Audio", this.audioEnabled ? "enabled" : "disabled");
  }

  /**
   * Gets the current audio enabled state.
   */
  public get isAudioEnabled(): boolean {
    return this.audioEnabled;
  }

  /**
   * Cleanup audio resources.
   */
  public destroy() {
    if (this.audioProcessor) {
      this.audioProcessor.disconnect();
      this.audioProcessor = null;
    }
    if (this.audioContext) {
      this.audioContext.close();
      this.audioContext = null;
    }
  }
}

class DiskController {
  private machine: Machine;
  private diskEnabled: boolean = false;
  private mountedDisks: Map<number, string> = new Map();
  private eventListeners: Array<{
    element: Element;
    event: string;
    handler: EventListener;
  }> = [];
  private systemHasDiskSupport: boolean = false;

  constructor(machine: Machine, hasDiskSupport: boolean = false) {
    console.log(
      `DiskController initialized with machine: ${machine}, hasDiskSupport: ${hasDiskSupport}`,
    );
    this.machine = machine;
    this.systemHasDiskSupport = hasDiskSupport;
    this.diskEnabled = hasDiskSupport; // If loaded with disk ROM, FDC is already enabled
    this.setupEventListeners();
  }

  public updateMachine(machine: Machine, hasDiskSupport: boolean) {
    this.machine = machine;
    this.systemHasDiskSupport = hasDiskSupport;
    this.diskEnabled = hasDiskSupport;
    // No need to enable disk system again if loaded with disk ROM
  }

  public cleanup() {
    // Remove all event listeners
    this.eventListeners.forEach(({ element, event, handler }) => {
      element.removeEventListener(event, handler);
    });
    this.eventListeners = [];
  }

  private setupEventListeners() {
    // Clean up any existing listeners first
    this.cleanup();

    // Setup listeners for both drives
    for (let drive = 0; drive < 2; drive++) {
      const mountButton = document.getElementById(`drive-${drive}-mount`);
      const ejectButton = document.getElementById(`drive-${drive}-eject`);
      const fileInput = document.getElementById(
        `drive-${drive}-file`,
      ) as HTMLInputElement;

      if (mountButton && fileInput) {
        const mountHandler = () => {
          console.log(`Mount button clicked for drive ${drive}`);
          fileInput.click();
        };
        mountButton.addEventListener("click", mountHandler);
        this.eventListeners.push({
          element: mountButton,
          event: "click",
          handler: mountHandler,
        });
      }

      if (ejectButton) {
        const ejectHandler = () => {
          this.ejectDisk(drive);
        };
        ejectButton.addEventListener("click", ejectHandler);
        this.eventListeners.push({
          element: ejectButton,
          event: "click",
          handler: ejectHandler,
        });
      }

      if (fileInput) {
        const changeHandler = (event: Event) => {
          const file = (event.target as HTMLInputElement).files?.[0];
          console.log(
            `File input changed for drive ${drive}, file: ${file?.name}`,
          );
          if (file) {
            this.mountDisk(drive, file);
          }
        };
        fileInput.addEventListener("change", changeHandler);
        this.eventListeners.push({
          element: fileInput,
          event: "change",
          handler: changeHandler,
        });
      } else {
        console.warn(
          `File input for drive ${drive} not found. Disk mounting will not work.`,
        );
      }
    }
  }

  private async mountDisk(drive: number, file: File) {
    try {
      console.log(
        `Mounting disk in drive ${drive}, system has disk support: ${this.systemHasDiskSupport}`,
      );

      // Only enable disk system if we don't have disk ROM support
      if (!this.diskEnabled && !this.systemHasDiskSupport) {
        console.log("Enabling disk system (no disk ROM loaded)");
        this.machine.enableDiskSystem();
        this.diskEnabled = true;
      }

      // Read the file
      const arrayBuffer = await file.arrayBuffer();
      const data = new Uint8Array(arrayBuffer);

      // Insert the disk
      this.machine.insertDisk(drive, data, file.name);
      this.mountedDisks.set(drive, file.name);

      // Update UI
      this.updateDriveStatus(drive, file.name);
    } catch (error) {
      console.error(`Failed to mount disk in drive ${drive}:`, error);
      alert(`Failed to mount disk: ${error}`);
    }
  }

  private ejectDisk(drive: number) {
    if (this.mountedDisks.has(drive)) {
      this.machine.ejectDisk(drive);
      this.mountedDisks.delete(drive);
      this.updateDriveStatus(drive, null);
    }
  }

  private updateDriveStatus(drive: number, filename: string | null) {
    const statusElement = document.getElementById(`drive-${drive}-status`);
    const ejectButton = document.getElementById(
      `drive-${drive}-eject`,
    ) as HTMLButtonElement;

    if (statusElement && ejectButton) {
      if (filename) {
        statusElement.textContent = filename;
        statusElement.classList.add("mounted");
        ejectButton.disabled = false;
      } else {
        statusElement.textContent = "Empty";
        statusElement.classList.remove("mounted");
        ejectButton.disabled = true;
      }
    }
  }
}

let currentApp: App | null = null;
let currentEmulator: Emulator | null = null;
let currentDiskController: DiskController | null = null;
let animationId: number | null = null;
let keyEventListenersAdded = false;

function setupMachine(machine: Machine, isRestart: boolean = false) {
  // Cancel any existing animation frame
  if (animationId !== null) {
    cancelAnimationFrame(animationId);
  }

  // Create new instances
  const emulator = new Emulator(machine);
  const renderer = new Renderer({ width: 256, height: 192 });
  const app = new App(renderer, emulator);

  // Get disk support status from SystemManager
  const systemManager = SystemManager.getInstance();
  const hasDiskSupport = systemManager.hasDiskSupport();

  console.log(
    `Setting up machine with disk support: ${hasDiskSupport}, isRestart: ${isRestart}`,
  );

  // Reuse existing DiskController on restart, create new one on initial setup
  if (currentDiskController) {
    currentDiskController.updateMachine(machine, hasDiskSupport);
  } else {
    currentDiskController = new DiskController(machine, hasDiskSupport);
  }

  // Store references
  currentApp = app;
  currentEmulator = emulator;
  (window as any).currentMachine = machine;

  // Set up audio toggle button
  const audioToggle = document.getElementById(
    "audio-toggle",
  ) as HTMLButtonElement;
  const audioStatus = document.getElementById("audio-status") as HTMLDivElement;

  if (audioToggle && audioStatus) {
    audioToggle.addEventListener("click", () => {
      emulator.toggleAudio();
      const isAudioEnabled = emulator.isAudioEnabled;
      audioStatus.textContent = isAudioEnabled ? "On" : "Off";
      audioToggle.textContent = isAudioEnabled ? "Disable" : "Enable";
    });
  }

  let lastTime = Date.now();

  // Only add event listeners once to avoid duplication
  if (!keyEventListenersAdded) {
    keyEventListenersAdded = true;

    window.addEventListener("keydown", (e) => {
      if (currentApp && currentApp.keyDown(e.code)) {
        if (currentEmulator && currentEmulator.isRunning()) {
          requestAnimationFrame(frame);
        }
        return;
      }
      const machine = (window as any).currentMachine ||
        currentEmulator?.machine;
      if (machine) {
        machine.keyDown(e.code);
      }
    });

    window.addEventListener("keyup", (e) => {
      if (currentApp && currentApp.keyUp(e.code)) {
        if (currentEmulator && currentEmulator.isRunning()) {
          requestAnimationFrame(frame);
        }
        return;
      }
      const machine = (window as any).currentMachine ||
        currentEmulator?.machine;
      if (machine) {
        machine.keyUp(e.code);
      }
    });
  }

  const frame = () => {
    const now = Date.now();
    const dt = (now - lastTime) / 1000;
    lastTime = now;
    app.frame(dt);

    if (emulator.isRunning()) {
      animationId = requestAnimationFrame(frame);
    }
  };

  animationId = requestAnimationFrame(frame);
}

async function setupBiosSelector() {
  const biosSelector = document.getElementById(
    "bios-selector",
  ) as HTMLSelectElement;
  const restartButton = document.getElementById(
    "bios-restart",
  ) as HTMLButtonElement;
  const biosFileInput = document.getElementById(
    "bios-file",
  ) as HTMLInputElement;

  if (!biosSelector || !restartButton || !biosFileInput) {
    console.error("BIOS selector elements not found");
    return;
  }

  // Handle BIOS selection change
  biosSelector.addEventListener("change", () => {
    if (biosSelector.value === "custom") {
      // If custom is selected but no ROM is loaded, trigger file picker
      if (!hasCustomRom()) {
        biosFileInput.click();
      }
    }
  });

  // Handle custom ROM file selection
  biosFileInput.addEventListener("change", async (event) => {
    const file = (event.target as HTMLInputElement).files?.[0];
    if (file) {
      try {
        const arrayBuffer = await file.arrayBuffer();
        const data = new Uint8Array(arrayBuffer);

        // Pad to 64KB if necessary
        let romData = data;
        if (data.length < 64 * 1024) {
          romData = new Uint8Array(64 * 1024);
          romData.fill(0xff);
          romData.set(data, 0);
        }

        setCustomRom(romData, file.name);

        // Update the select option text to show the filename
        const customOption = biosSelector.querySelector(
          'option[value="custom"]',
        );
        if (customOption) {
          customOption.textContent = `Custom: ${file.name}`;
        }

        console.log(
          `Custom ROM loaded: ${file.name} (${data.length} bytes, padded to ${romData.length} bytes)`,
        );
      } catch (error) {
        console.error("Failed to load custom ROM:", error);
        alert(`Failed to load custom ROM: ${error}`);
        // Reset selection if loading failed
        biosSelector.value = "expert";
      }
    } else {
      // User cancelled, reset selection if no custom ROM is loaded
      if (!hasCustomRom()) {
        biosSelector.value = "expert";
      }
    }
  });

  // Handle BIOS restart button
  restartButton.addEventListener("click", async () => {
    const selectedBiosId = biosSelector.value;
    const biosOption = getBiosById(selectedBiosId);

    if (!biosOption) {
      console.error("Invalid BIOS selection:", selectedBiosId);
      return;
    }

    // Special handling for custom ROM
    if (selectedBiosId === "custom" && !hasCustomRom()) {
      alert("Please select a custom ROM file first");
      biosFileInput.click();
      return;
    }

    try {
      restartButton.disabled = true;
      restartButton.textContent = "Loading...";

      // Load the selected BIOS
      const biosData = await biosOption.loader();

      // Change BIOS in SystemManager
      const systemManager = SystemManager.getInstance();
      await systemManager.changeBios(biosData, selectedBiosId);

      restartButton.textContent = "Restart";
      restartButton.disabled = false;
    } catch (error) {
      console.error("Failed to load BIOS:", error);
      restartButton.textContent = "Error!";
      setTimeout(() => {
        restartButton.textContent = "Restart";
        restartButton.disabled = false;
      }, 2000);
    }
  });
}

async function main() {
  // Initialize with default BIOS (Expert)
  const defaultBios = getBiosById("expert");
  if (!defaultBios) {
    throw new Error("Default BIOS not found");
  }

  const biosData = await defaultBios.loader();
  const systemManager = SystemManager.getInstance(biosData, "expert");

  // Set up machine restart callback
  systemManager.setOnMachineRestart((machine) => {
    console.log("Machine restarted with new configuration");
    setupMachine(machine, true);
  });

  // Set up BIOS selector
  await setupBiosSelector();

  // Initial setup
  setupMachine(systemManager.getMachine(), false);
}

function onLoad() {
  init().then(main);
}

window.addEventListener("DOMContentLoaded", onLoad, false);
