import { writable, derived, get } from 'svelte/store';

// ============================================================================
// Tipos
// ============================================================================

export interface ImageInfo {
  width: number;
  height: number;
  original_size: number;
  preview_base64: string;
}

export interface OptimizationResult {
  preview_base64: string;
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

export const originalImage = writable<ImageInfo | null>(null);
export const optimizedPreview = writable<OptimizationResult | null>(null);
export const isProcessing = writable(false);
export const isLoading = writable(false);

// Configuración actual del encoder
export const encoderOptions = writable<EncoderOptions>({
  encoder_name: "mozjpeg",
  options: {
    quality: 75
  }
});

// Store para tracking de operaciones cancelables (futuro)
export const currentOperationId = writable<string | null>(null);

// ============================================================================
// Derived Helpers (Estadísticas formateadas)
// ============================================================================

function formatBytes(bytes: number, decimals = 2): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const dm = decimals < 0 ? 0 : decimals;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(dm)) + ' ' + sizes[i];
}

export const originalSizeFormatted = derived(originalImage, $img => 
  $img ? formatBytes($img.original_size) : '0 B'
);

export const optimizedSizeFormatted = derived(optimizedPreview, $prev => 
  $prev ? formatBytes($prev.optimized_size) : '0 B'
);

export const savingsFormatted = derived(optimizedPreview, $prev => 
  $prev ? `${$prev.savings_percent.toFixed(1)}%` : '0%'
);

// Derived store para dimensiones originales
export const originalDimensions = derived(originalImage, $img => 
  $img ? { width: $img.width, height: $img.height } : null
);

// ============================================================================
// Utility Functions
// ============================================================================

/**
 * Limpia los stores cuando se descarga una imagen
 */
export function resetStores(): void {
  originalImage.set(null);
  optimizedPreview.set(null);
  isProcessing.set(false);
  isLoading.set(false);
  currentOperationId.set(null);
}

/**
 * Revoca un Blob URL para liberar memoria
 * (Preparado para cuando migremos de Base64 a Blob URLs)
 */
export function revokePreviewUrl(url: string | null): void {
  if (url && url.startsWith("blob:")) {
    URL.revokeObjectURL(url);
  }
}

/**
 * Genera un ID único para operaciones
 */
export function generateOperationId(): string {
  return `op-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
}
