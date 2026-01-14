import { writable, derived, get } from "svelte/store";

// ============================================================================
// Tipos - Sistema Canvas-based (como Squoosh)
// ============================================================================

/**
 * Información básica de la imagen (sin datos de píxeles)
 * Se obtiene al cargar la imagen
 */
export interface ImageInfo {
  width: number;
  height: number;
  original_size: number;
  name: string;
}

/**
 * Datos raw de imagen para canvas rendering
 * RGBA: 4 bytes por píxel
 */
export interface ImageDataRaw {
  width: number;
  height: number;
  /** RGBA raw bytes - Uint8Array de Rust Vec<u8> */
  data: number[];
}

/**
 * Resultado de optimización (sin preview - se obtiene separadamente)
 */
export interface OptimizationResult {
  optimized_size: number;
  savings_percent: number;
  mime_type: string;
  extension: string;
}

export interface OptimizationMetadata {
  optimized_size: number;
  savings_percent: number;
  mime_type: string;
  extension: string;
}

export interface ResizeOptions {
  width: number;
  height: number;
  filter: string;
}

export interface QuantizeOptions {
  num_colors: number;
  dither: number;
}

export interface OptimizationRequest {
  encoder_name: string;
  options: Record<string, unknown>;
  resize?: ResizeOptions;
  quantize?: QuantizeOptions;
}

export interface EncoderOptions {
  encoder_name: string;
  options: Record<string, unknown>;
}

// ============================================================================
// Stores
// ============================================================================

/** Información de la imagen original (dimensiones, tamaño) */
export const originalImageInfo = writable<ImageInfo | null>(null);

/** Resultado de la última optimización (tamaño, savings) */
export const optimizationResult = writable<OptimizationResult | null>(null);

/** Estado de carga/procesamiento */
export const isProcessing = writable(false);
export const isLoading = writable(false);

/** Estado del canvas original - true cuando está listo para mostrar */
export const originalCanvasReady = writable(false);

/** Estado del canvas optimizado - true cuando está listo para mostrar */
export const optimizedCanvasReady = writable(false);

/** Configuración actual del encoder */
export const encoderOptions = writable<EncoderOptions>({
  encoder_name: "mozjpeg",
  options: {
    quality: 75,
  },
});

/** Store para tracking de operaciones cancelables (futuro) */
export const currentOperationId = writable<string | null>(null);

/** Store para comunicar archivos soltados (Drag & Drop) desde CompareSlider a ControlPanel */
export const droppedFile = writable<string | null>(null);

// ============================================================================
// Derived Helpers (Estadísticas formateadas)
// ============================================================================

function formatBytes(bytes: number, decimals = 2): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const dm = decimals < 0 ? 0 : decimals;
  const sizes = ["B", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(dm)) + " " + sizes[i];
}

export const originalSizeFormatted = derived(originalImageInfo, ($img) =>
  $img ? formatBytes($img.original_size) : "0 B"
);

export const optimizedSizeFormatted = derived(optimizationResult, ($prev) =>
  $prev ? formatBytes($prev.optimized_size) : "0 B"
);

export const savingsFormatted = derived(optimizationResult, ($prev) =>
  $prev ? `${$prev.savings_percent.toFixed(1)}%` : "0%"
);

export const originalDimensions = derived(originalImageInfo, ($img) =>
  $img ? { width: $img.width, height: $img.height } : null
);

// ============================================================================
// Utility Functions
// ============================================================================

/**
 * Limpia los stores cuando se descarga una imagen
 */
export function resetStores(): void {
  originalImageInfo.set(null);
  optimizationResult.set(null);
  isProcessing.set(false);
  isLoading.set(false);
  currentOperationId.set(null);
  originalCanvasReady.set(false);
  optimizedCanvasReady.set(false);
}

/**
 * Genera un ID único para operaciones
 */
export function generateOperationId(): string {
  return `op-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
}

/**
 * Convierte ImageDataRaw de Rust a ImageData de Canvas
 * Esta función es crítica para el renderizado full-resolution
 */
export function rawToImageData(raw: ImageDataRaw): ImageData {
  // Rust envía Vec<u8> que llega como number[]
  // Convertimos a Uint8ClampedArray para ImageData
  const clampedArray = new Uint8ClampedArray(raw.data);
  return new ImageData(clampedArray, raw.width, raw.height);
}

/**
 * Dibuja ImageData en un canvas de forma optimizada
 * Similar a drawDataToCanvas de Squoosh
 */
export function drawImageDataToCanvas(
  canvas: HTMLCanvasElement,
  imageData: ImageData
): void {
  // Asegurar dimensiones correctas
  if (canvas.width !== imageData.width || canvas.height !== imageData.height) {
    canvas.width = imageData.width;
    canvas.height = imageData.height;
  }

  const ctx = canvas.getContext("2d", {
    alpha: true,
    desynchronized: true, // Mejor performance
  });

  if (ctx) {
    ctx.putImageData(imageData, 0, 0);
  }
}
