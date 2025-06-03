import ROMS from "./roms.js";

export interface BiosOption {
  id: string;
  name: string;
  loader: () => Promise<Uint8Array> | Uint8Array;
}

// Store for custom ROM data
let customRomData: Uint8Array | null = null;
let customRomName: string = "Custom ROM";

// Helper function to load and combine CBIOS ROMs
async function loadCBIOS(): Promise<Uint8Array> {
  try {
    // Load both ROM files
    const [mainResponse, logoResponse] = await Promise.all([
      fetch('./cbios/cbios_main_msx1.rom'),
      fetch('./cbios/cbios_logo_msx1.rom')
    ]);
    
    if (!mainResponse.ok || !logoResponse.ok) {
      throw new Error('Failed to load CBIOS ROMs');
    }
    
    const [mainData, logoData] = await Promise.all([
      mainResponse.arrayBuffer(),
      logoResponse.arrayBuffer()
    ]);
    
    // Create a 64KB ROM image
    const combined = new Uint8Array(64 * 1024);
    
    // Fill with 0xFF (empty ROM pattern)
    combined.fill(0xFF);
    
    const mainArray = new Uint8Array(mainData);
    const logoArray = new Uint8Array(logoData);
    
    // Place main ROM at 0x0000-0x7FFF (32KB)
    combined.set(mainArray, 0);
    
    // Place logo ROM at 0x8000-0xBFFF (16KB)
    combined.set(logoArray, 0x8000);
    
    console.log(`Loaded CBIOS: main=${mainArray.length} bytes, logo=${logoArray.length} bytes, total=${combined.length} bytes`);
    
    return combined;
  } catch (error) {
    console.error('Error loading CBIOS:', error);
    throw error;
  }
}

// Helper function to load custom ROM
async function loadCustomRom(): Promise<Uint8Array> {
  if (!customRomData) {
    throw new Error('No custom ROM loaded');
  }
  return customRomData;
}

// Available BIOS options
export const BIOS_OPTIONS: BiosOption[] = [
  {
    id: 'expert',
    name: 'Expert 1.1 (Brazilian)',
    loader: () => ROMS.expert
  },
  {
    id: 'hotbit',
    name: 'Hotbit HB-8000',
    loader: () => ROMS.hotbit
  },
  {
    id: 'cbios',
    name: 'CBIOS MSX1',
    loader: loadCBIOS
  },
  {
    id: 'custom',
    name: 'Custom ROM',
    loader: loadCustomRom
  }
];

export function getBiosById(id: string): BiosOption | undefined {
  const bios = BIOS_OPTIONS.find(bios => bios.id === id);
  // Update custom ROM name if it's been set
  if (bios && bios.id === 'custom' && customRomName !== "Custom ROM") {
    bios.name = customRomName;
  }
  return bios;
}

// Function to set custom ROM data
export function setCustomRom(data: Uint8Array, filename: string) {
  customRomData = data;
  customRomName = filename;
  // Update the BIOS option name
  const customOption = BIOS_OPTIONS.find(bios => bios.id === 'custom');
  if (customOption) {
    customOption.name = filename;
  }
}

// Function to check if custom ROM is loaded
export function hasCustomRom(): boolean {
  return customRomData !== null;
}