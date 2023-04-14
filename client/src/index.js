import init, { Machine } from "../pkg/wasmsx.js";
import ROMS from "./roms.js";

await init();

const machine = new Machine(ROMS.hotbit);

let running = false;
let lastTime = window.performance.now();

window.addEventListener("keydown", (e) => {
  if (e.code === "Space") {
    if (running) {
      pause();
    } else {
      start();
    }
    return;
  }

  if (e.code === "Escape") {
    pause();
    return;
  }

  if (e.code === "KeyN") {
    window.requestAnimationFrame(frame);
    return;
  }

  if (e.code === "KeyD") {
    console.log("ram", machine.ram);

    const rows = [];
    const data = machine.ram;
    for (let i = 0; i < data.length; i += 16) {
      const row = [];
      const chars = [];
      for (let j = 0; j < 16; j++) {
        const value = data[i + j];
        row.push(value.toString(16).padStart(2, "0"));
        chars.push(
          value >= 32 && value <= 126 ? String.fromCharCode(value) : "."
        );
      }
      rows.push(row.join(" ") + "  " + chars.join(""));
    }

    document.getElementById("dump").innerHTML = rows
      .map((r) => `<div>${r}</div>`)
      .join("");

    return;
  }

  console.log("keydown", e.code);
});

function frame() {
  const currentTime = performance.now();
  // const delta = Math.max(0, (currentTime - lastTime) / 1000);
  lastTime = currentTime;

  machine.step();

  console.log("pc", `0x${machine.pc.toString(16).padStart(4, "0")}`);
  if (running) {
    window.requestAnimationFrame(frame);
  }
}

function pause() {
  running = false;
}

function start() {
  window.requestAnimationFrame(frame);
  running = true;
}
