import init, { Machine } from "../pkg/wasmsx.js";
import ROMS from "./roms.js";
const PROCESSOR_RATE = 1024 * 1024 * 3.5;
class Renderer {
    screen;
    data;
    ctx;
    screenImageData;
    constructor(screen) {
        this.screen = document.getElementById("screen");
        this.data = document.getElementById("data");
        const ctx = this.screen.getContext("2d");
        if (!ctx) {
            throw new Error("Could not get canvas context");
        }
        this.ctx = ctx;
        this.screenImageData = this.ctx.createImageData(screen.width, screen.height);
    }
    renderScreen(buffer) {
        const pixels = this.screenImageData.data;
        for (let y = 0; y < 192; y++) {
            for (let x = 0; x < 256; x++) {
                const colorOffset = y * 256 + x;
                const color = buffer[colorOffset];
                if (!color)
                    continue;
                const colorBytes = new Uint8Array(4);
                colorBytes[0] = color;
                colorBytes[1] = color;
                colorBytes[2] = color;
                colorBytes[3] = 255;
                pixels.set(colorBytes, colorOffset * 4);
            }
        }
        this.ctx.putImageData(this.screenImageData, 0, 0);
    }
    renderData(data) {
        console.log("pc", data.pc.toString(16), data.display_mode);
        const pc = document.getElementById("pc");
        if (pc) {
            pc.innerText = data.pc.toString(16);
        }
    }
}
class App {
    renderer;
    emulator;
    constructor(renderer, emulator) {
        this.renderer = renderer;
        this.emulator = emulator;
    }
    keyDown(key) {
        this.emulator.keyDown(key);
    }
    frame(dt) {
        if (dt > 0.2) {
            // console.log(`${dt} seconds behind`);
        }
        else {
            this.emulator.run(dt);
        }
        this.emulator.run(dt);
        this.renderer.renderScreen(this.emulator.getScreen());
        this.renderer.renderData(this.emulator.getData());
    }
}
class Emulator {
    keys;
    timeBudget;
    machine;
    constructor(machine) {
        this.machine = machine;
        this.timeBudget = 0;
        this.keys = {};
    }
    run(dt) {
        this.timeBudget += dt;
        const cycles = Math.floor(this.timeBudget * PROCESSOR_RATE);
        const cycleTime = cycles / PROCESSOR_RATE;
        this.timeBudget -= cycleTime;
        for (let i = 0; i < cycles; i++) {
            this.machine.step();
        }
    }
    getScreen() {
        return this.machine.screen();
    }
    getData() {
        return { pc: this.machine.pc, display_mode: this.machine.display_mode };
    }
    screen() {
        // return this.machine.screen();
    }
    keyDown(key) {
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
