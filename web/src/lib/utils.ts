import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"
import init from "@pkg/pyhyeon"

let wasmInitialized = false
let wasmInitPromise: Promise<void> | null = null

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

export async function initWasm() {
  if (wasmInitialized) return
  if (wasmInitPromise) return wasmInitPromise
  
  wasmInitPromise = (async () => {
    try {
      await init()
      wasmInitialized = true
      console.log("WASM initialized successfully")
    } catch (error) {
      console.error("Failed to initialize WASM:", error)
      wasmInitPromise = null
      throw error
    }
  })()
  
  return wasmInitPromise
}

export function isWasmReady() {
  return wasmInitialized
}