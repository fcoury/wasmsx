import init, { Machine } from "../pkg/wasmsx.js";
import ROMS from "./roms.js";

export interface DiskRomConfig {
  biosRom: Uint8Array;
  diskRom: Uint8Array | null;
}

export class SystemManager {
  private static instance: SystemManager | null = null;
  private machine: Machine | null = null;
  private diskRomData: Uint8Array | null = null;
  private biosRomData: Uint8Array;
  private currentBiosId: string = 'expert';
  private onMachineRestart?: (machine: Machine) => void;
  private _hasDiskSupport: boolean = false;

  constructor(biosRomData: Uint8Array, biosId: string = 'expert') {
    this.biosRomData = biosRomData;
    this.currentBiosId = biosId;
    this.setupDiskRomLoader();
  }

  static getInstance(biosRomData?: Uint8Array, biosId?: string): SystemManager {
    if (!SystemManager.instance) {
      if (!biosRomData) {
        throw new Error("BIOS ROM data required for initial setup");
      }
      SystemManager.instance = new SystemManager(biosRomData, biosId);
    }
    return SystemManager.instance;
  }
  
  async changeBios(biosRomData: Uint8Array, biosId: string) {
    this.biosRomData = biosRomData;
    this.currentBiosId = biosId;
    
    // Restart the machine with new BIOS
    this.restartMachine();
  }
  
  getCurrentBiosId(): string {
    return this.currentBiosId;
  }

  private setupDiskRomLoader() {
    const fileInput = document.getElementById('disk-rom-file') as HTMLInputElement;
    const loadButton = document.getElementById('disk-rom-load');
    const statusElement = document.getElementById('disk-rom-status');

    loadButton?.addEventListener('click', () => {
      fileInput.click();
    });

    fileInput?.addEventListener('change', async (event) => {
      const file = (event.target as HTMLInputElement).files?.[0];
      if (file) {
        try {
          const arrayBuffer = await file.arrayBuffer();
          this.diskRomData = new Uint8Array(arrayBuffer);
          
          if (statusElement) {
            statusElement.textContent = file.name;
            statusElement.classList.add('mounted');
          }
          
          // Restart the machine with the new configuration
          this.restartMachine();
        } catch (error) {
          console.error('Failed to load disk ROM:', error);
          alert(`Failed to load disk ROM: ${error}`);
        }
      }
    });
  }

  getMachine(): Machine {
    if (!this.machine) {
      this.createMachine();
    }
    return this.machine!;
  }

  private createMachine() {
    if (this.diskRomData) {
      console.log("Creating machine with disk ROM support");
      console.log("BIOS ROM size:", this.biosRomData.length, "bytes");
      console.log("Disk ROM size:", this.diskRomData.length, "bytes");
      console.log("Disk ROM size (hex):", "0x" + this.diskRomData.length.toString(16));
      
      // Check if disk ROM size is valid
      if (this.diskRomData.length !== 0x4000 && this.diskRomData.length !== 0x8000 && this.diskRomData.length !== 0x10000) {
        console.warn("Warning: Disk ROM size is not standard (16KB, 32KB, or 64KB)");
      }
      
      try {
        this.machine = Machine.newWithDisk(this.biosRomData, this.diskRomData);
        this._hasDiskSupport = true;
      } catch (error) {
        console.error("Failed to create machine with disk ROM:", error);
        throw error;
      }
    } else {
      console.log("Creating machine without disk ROM");
      this.machine = new Machine(this.biosRomData);
      this._hasDiskSupport = false;
    }
  }

  private restartMachine() {
    // Create new machine with updated configuration
    this.createMachine();
    
    // Show restart message
    const canvas = document.getElementById('screen') as HTMLCanvasElement;
    const ctx = canvas.getContext('2d');
    if (ctx) {
      ctx.fillStyle = '#000';
      ctx.fillRect(0, 0, canvas.width, canvas.height);
      ctx.fillStyle = '#FFF';
      ctx.font = '16px monospace';
      ctx.textAlign = 'center';
      ctx.fillText('Restarting with Disk ROM...', canvas.width / 2, canvas.height / 2);
    }
    
    // Notify listeners that machine has been restarted
    setTimeout(() => {
      if (this.onMachineRestart && this.machine) {
        this.onMachineRestart(this.machine);
      }
    }, 500);
  }

  setOnMachineRestart(callback: (machine: Machine) => void) {
    this.onMachineRestart = callback;
  }

  hasDiskRom(): boolean {
    return this.diskRomData !== null;
  }
  
  hasDiskSupport(): boolean {
    return this._hasDiskSupport;
  }
}