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

const PROCESSOR_RATE = 1024 * 1024 * 3.5;
const PALETTE = [
  // MSX1 color palette
  0x000000, 0x0000aa, 0x00aa00, 0x00aaaa, 0xaa0000, 0xaa00aa, 0xaa5500,
  0xaaaaaa, 0x555555, 0x5555ff, 0x55ff55, 0x55ffff, 0xff5555, 0xff55ff,
  0xffff55, 0xffffff,
];

class Renderer {
  private screen: HTMLCanvasElement;
  private data: HTMLDivElement;

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
    this.data = document.getElementById("data") as HTMLDivElement;

    const ctx = this.screen.getContext("2d");
    if (!ctx) {
      throw new Error("Could not get canvas context");
    }

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

  /**
   * Renders the internal emulator data for debugging.
   *
   * @param {Object} data - The emulator data
   * @param {number} data.pc - The program counter value
   * @param {string} data.display_mode - The display mode
   */
  public renderData(data: { pc: number; display_mode: string }) {
    this.data.innerText = `Display Mode: ${data.display_mode} PC: 0x${data.pc
      .toString(16)
      .padStart(4, "0")} `;
  }
}

class App {
  private renderer: Renderer;
  private emulator: Emulator;

  /**
   * Constructs a new App instance.
   * @param {Renderer} renderer - The Renderer instance
   * @param {Emulator} emulator - The Emulator instance
   */
  constructor(renderer: Renderer, emulator: Emulator) {
    this.renderer = renderer;
    this.emulator = emulator;
  }

  /**
   * Handles keyDown events.
   * @param {number} key - The key code
   */
  public keyDown(key: number) {
    this.emulator.keyDown(key);
  }

  /**
   * Handles a single frame of the emulator.
   * @param {number} dt - The delta time since the last frame
   */
  public frame(dt: number) {
    if (dt > 0.2) {
      // console.log(`${dt} seconds behind`);
    } else {
      this.emulator.run(dt);
    }
    this.emulator.run(dt);
    this.renderer.renderScreen(this.emulator.getScreen());
    this.renderer.renderData(this.emulator.getData());
  }
}

class Emulator {
  private keys: { [key: string]: boolean };
  private timeBudget: number;
  private machine: Machine;

  /**
   * Constructs a new Emulator instance.
   * @param {Machine} machine - The Machine instance from the wasm module
   */
  constructor(machine: Machine) {
    this.machine = machine;
    this.timeBudget = 0;
    this.keys = {};
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

    for (let i = 0; i < cycles; i++) {
      this.machine.step();
    }
  }

  /**
   * Returns the screen buffer data.
   * @returns {Uint8Array} The screen buffer data
   */
  public getScreen(): Uint8Array {
    return this.machine.screen();
  }

  /**
   * Returns the emulator debug data.
   * @returns {Object} The emulator data
   */
  public getData(): { pc: number; display_mode: string } {
    return { pc: this.machine.pc, display_mode: this.machine.display_mode };
  }

  /**
   * Handles keyDown events.
   * @param {number} key - The key code
   */
  public keyDown(key: number) {
    // this.machine.key_down(key);
  }
}

function onLoad() {
  console.log("onload");
  init().then(main);
}

function main() {
  const machine = new Machine(ROMS.hotbit);
  const emulator = new Emulator(machine);
  const renderer = new Renderer({ width: 256, height: 192 });
  const app = new App(renderer, emulator);
  let lastTime = Date.now();

  const frame = () => {
    const now = Date.now();
    const dt = (now - lastTime) / 1000;
    lastTime = now;
    app.frame(dt);
    requestAnimationFrame(frame);
  };

  requestAnimationFrame(frame);
}

onLoad();
