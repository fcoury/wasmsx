import { useEffect, useState } from "react";
import init, { Machine } from "wasmsx";
import "./App.css";
import Hexdump from "./components/Hexdump";
import ROMS from "./roms";

export default function App() {
  const [machine, setMachine] = useState<Machine | null>(null);
  const [pc, setPc] = useState<number | null>(null);
  const [ram, setRam] = useState<Uint8Array | null>(null);

  const handleMachineChanged = () => {
    console.log("machine changed", machine);
    // if (!machine) return;

    // setPc(machine.pc);
    // setRam(machine.ram);
  };

  useEffect(() => {
    init().then(() => {
      console.log("init");
      const machine = new Machine(ROMS.hotbit, handleMachineChanged);
      setMachine(machine);
      setPc(machine.pc);
      setRam(machine.ram);

      console.log("machine", machine);
    });
  }, []);

  const handleStep = () => {
    console.log("handleStep", machine);
    if (machine) {
      console.log("machine", machine.pc);
      machine.step();
      setPc(machine.pc);
      setRam(machine.ram);
      console.log("machine", machine.pc);
    }
  };

  return (
    <div className="App">
      <div>
        <button onClick={handleStep}>Step</button>
      </div>
      <div>
        {pc && (
          <div>
            <div>PC: {pc.toString(16).padStart(4, "0")}</div>
          </div>
        )}
      </div>
      {ram && <Hexdump data={ram} />}
    </div>
  );
}
