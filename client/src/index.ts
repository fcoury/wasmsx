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
import ROMS from "./roms.js";

const PROCESSOR_RATE = 3.579 * 1000 * 1000;
const PALETTE = [
  0x000000, 0x010101, 0x3eb849, 0x74d07d, 0x5955e0, 0x8076f1, 0xb95e51,
  0x65dbef, 0xdb6559, 0xff897d, 0xccc35e, 0xded087, 0x3aa241, 0xb766b5,
  0xcccccc, 0xffffff,
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
      screen.height
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
        const color = buffer[colorOffset];
        if (!color) continue;
        const colorBytes = new Uint8Array(4);
        const paletteColor = PALETTE[color] || 0xffffff;

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
    return this.emulator.keyDown(key);
  }

  /**
   * Handles keyUp events.
   * @param {string} key - The key code
   * @returns {boolean} Whether the key was handled
   */
  public keyUp(key: string): boolean {
    const handled = this.emulator.keyUp(key);
    return handled;
  }

  /**
   * Handles a single frame of the emulator.
   * @param {number} dt - The delta time since the last frame
   */
  public frame(dt: number) {
    if (dt > 0.2) {
      console.log(`${dt} seconds behind`);
    } else {
      this.emulator.run(dt);
      this.renderer.renderScreen(this.emulator.getScreen());
      if (this.debugVisible) {
        this.emulator.renderState();
        this.emulator.renderVRAM();
      }
    }
  }
}

class Emulator {
  private timeBudget: number;
  private machine: Machine;
  private running: boolean;
  private vram: HTMLDivElement;
  private state: HTMLDivElement;

  /**
   * Constructs a new Emulator instance.
   * @param {Machine} machine - The Machine instance from the wasm module
   */
  constructor(machine: Machine) {
    this.running = true;
    this.machine = machine;
    this.timeBudget = 0;
    this.vram = document.getElementById("vram") as HTMLDivElement;
    this.state = document.getElementById("state") as HTMLDivElement;
  }

  /**
   * Runs the emulator for a specified duration.
   * @param {number} dt - The delta time since the last run
   */
  public run(dt: number) {
    this.timeBudget += dt;
    const cycles = Math.floor(this.timeBudget * PROCESSOR_RATE);
    const cycleTime = cycles / PROCESSOR_RATE;
    this.timeBudget -= cycleTime;
    this.machine.step_for(cycles);
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
   * Renders div with PC and display mode.
   **/
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
          value >= 32 && value <= 126 ? String.fromCharCode(value) : "."
        );
      }
      const addr = i.toString(16).padStart(4, "0");
      rows.push(addr + ":  " + row.join(" ") + "  " + chars.join(""));
    }

    this.vram.innerHTML = `<pre>${rows
      .map((row) => `<div>${row}</div>`)
      .join("")}</pre>`;
  }

  /**
   * Handles keyDown events.
   * @param {string} key - The key code
   * @returns {boolean} Whether the key was handled
   */
  public keyDown(key: string): boolean {
    console.log("key", key);
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
}

function main() {
  const machine = new Machine(ROMS.expert);
  const emulator = new Emulator(machine);
  const renderer = new Renderer({ width: 256, height: 192 });
  const app = new App(renderer, emulator);
  let lastTime = Date.now();

  window.addEventListener("keydown", (e) => {
    if (app.keyDown(e.code)) {
      if (emulator.isRunning()) {
        requestAnimationFrame(frame);
      }
      return;
    }
    machine.keyDown(e.code);
  });

  window.addEventListener("keyup", (e) => {
    if (app.keyUp(e.code)) {
      if (emulator.isRunning()) {
        requestAnimationFrame(frame);
      }
      return;
    }
    machine.keyUp(e.code);
  });

  const frame = () => {
    const now = Date.now();
    const dt = (now - lastTime) / 1000;
    lastTime = now;
    app.frame(dt);

    if (emulator.isRunning()) {
      requestAnimationFrame(frame);
    }
  };

  requestAnimationFrame(frame);
}

function onLoad() {
  init().then(main);
}

window.addEventListener("DOMContentLoaded", onLoad, false);
