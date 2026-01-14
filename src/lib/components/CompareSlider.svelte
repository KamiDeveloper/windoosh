<!--
  CompareSlider.svelte
  Sistema de previsualización Canvas-based (como Squoosh)
  
  Características:
  - Renderizado Canvas a resolución COMPLETA (no Base64 reducido)
  - Pinch-zoom sincronizado entre ambos canvas
  - CSS transforms para zoom/pan (no re-escala píxeles)
  - ImageData directo desde Rust (RGBA raw bytes)
-->
<script lang="ts">
  import { onMount, onDestroy, createEventDispatcher } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import {
    originalImageInfo,
    optimizationResult,
    isLoading,
    isProcessing,
    rawToImageData,
    droppedFile,
    type ImageDataRaw,
    resetStores,
  } from "$lib/stores/imageStore";
  import { generalIcons } from "$lib/icons";

  // Tipos para eventos de drag & drop en Tauri 2.0
  interface TauriDropPayload {
    type: "drop";
    paths: string[];
    position: { x: number; y: number };
  }

  const dispatch = createEventDispatcher<{ openFile: void }>();

  // Canvas refs
  let canvasOriginal: HTMLCanvasElement;
  let canvasOptimized: HTMLCanvasElement;
  let container: HTMLDivElement;

  // Slider position (0-100)
  let sliderPosition = 50;

  // Transform State (aplicado via CSS, NO re-escala píxeles)
  let scale = 1;
  let translateX = 0;
  let translateY = 0;

  // Interaction State
  let isDraggingSlider = false;
  let isPanning = false;
  let panStartX = 0;
  let panStartY = 0;
  let panStartTranslateX = 0;
  let panStartTranslateY = 0;

  // Image dimensions (for proper sizing)
  let imageWidth = 0;
  let imageHeight = 0;

  // Drag & Drop State
  let isDragOver = false;
  let unlistenDrop: UnlistenFn | null = null;
  let unlistenHover: UnlistenFn | null = null;
  let unlistenLeave: UnlistenFn | null = null;

  // Extensiones válidas de imagen
  const validExtensions = [".png", ".jpg", ".jpeg", ".webp", ".gif", ".bmp"];

  function isValidImagePath(path: string): boolean {
    const lower = path.toLowerCase();
    return validExtensions.some((ext) => lower.endsWith(ext));
  }

  // ========================================================================
  // Lifecycle
  // ========================================================================

  onMount(async () => {
    try {
      // Tauri drag & drop events
      unlistenHover = await listen("tauri://drag-over", () => {
        isDragOver = true;
      });

      unlistenLeave = await listen("tauri://drag-leave", () => {
        isDragOver = false;
      });

      unlistenDrop = await listen<TauriDropPayload>(
        "tauri://drag-drop",
        (event) => {
          isDragOver = false;
          const paths = event.payload.paths;
          if (paths && paths.length > 0) {
            const filePath = paths[0];
            if (isValidImagePath(filePath)) {
              // Updated: Use store instead of window event
              droppedFile.set(filePath);
            }
          }
        }
      );
    } catch (e) {
      // En dev sin Tauri, ignorar
    }
  });

  onDestroy(() => {
    if (unlistenDrop) unlistenDrop();
    if (unlistenHover) unlistenHover();
    if (unlistenLeave) unlistenLeave();
  });

  // ========================================================================
  // Canvas Rendering - Full Resolution
  // ========================================================================

  /**
   * Carga y dibuja la imagen original en el canvas
   * Se llama cuando cambia originalImageInfo
   */
  async function loadOriginalCanvas() {
    if (!$originalImageInfo || !canvasOriginal) return;

    try {
      // Obtener raw RGBA data desde Rust
      const rawData = await invoke<ImageDataRaw>("get_original_image_data");

      // Convertir a ImageData y dibujar
      const imageData = rawToImageData(rawData);
      drawToCanvas(canvasOriginal, imageData);

      // Actualizar dimensiones
      imageWidth = rawData.width;
      imageHeight = rawData.height;

      // Reset view cuando carga nueva imagen
      resetView();
    } catch (err) {
      console.error("Error cargando canvas original:", err);
    }
  }

  /**
   * Carga y dibuja la imagen procesada en el canvas
   * Se llama cuando cambia optimizationResult
   */
  async function loadProcessedCanvas() {
    if (!$optimizationResult || !canvasOptimized) return;

    try {
      // Obtener raw RGBA data desde Rust
      const rawData = await invoke<ImageDataRaw>("get_processed_image_data");

      // Convertir a ImageData y dibujar
      const imageData = rawToImageData(rawData);
      drawToCanvas(canvasOptimized, imageData);

      // Actualizar dimensiones (puede cambiar si hay resize)
      imageWidth = rawData.width;
      imageHeight = rawData.height;
    } catch (err) {
      console.error("Error cargando canvas procesado:", err);
    }
  }

  /**
   * Dibuja ImageData en un canvas a resolución completa
   * Similar a drawDataToCanvas de Squoosh
   */
  function drawToCanvas(canvas: HTMLCanvasElement, imageData: ImageData): void {
    // Establecer dimensiones del canvas al tamaño real de la imagen
    canvas.width = imageData.width;
    canvas.height = imageData.height;

    const ctx = canvas.getContext("2d", {
      alpha: true,
      desynchronized: true, // Mejor performance
    });

    if (ctx) {
      ctx.imageSmoothingEnabled = true;
      ctx.imageSmoothingQuality = "high";
      ctx.putImageData(imageData, 0, 0);
    }
  }

  // Reactive: cargar canvas cuando cambian los stores
  $: if ($originalImageInfo && canvasOriginal) {
    loadOriginalCanvas();
  }

  $: if ($optimizationResult && canvasOptimized) {
    loadProcessedCanvas();
  }

  // ========================================================================
  // Transform Style (CSS-based zoom/pan)
  // ========================================================================

  $: transformStyle = `
    transform: translate(${translateX}px, ${translateY}px) scale(${scale});
    transform-origin: 0 0;
  `;

  // ========================================================================
  // Slider Logic
  // ========================================================================

  function handleMouseDown(e: MouseEvent) {
    const target = e.target as Element;

    if (target.closest(".slider-handle")) {
      isDraggingSlider = true;
      updateSliderPosition(e);
      e.preventDefault();
    } else if ($originalImageInfo && $optimizationResult) {
      // Pan mode
      isPanning = true;
      panStartX = e.clientX;
      panStartY = e.clientY;
      panStartTranslateX = translateX;
      panStartTranslateY = translateY;
      if (container) container.style.cursor = "grabbing";
    }
  }

  function handleMouseMove(e: MouseEvent) {
    if (isDraggingSlider) {
      updateSliderPosition(e);
    } else if (isPanning) {
      translateX = panStartTranslateX + (e.clientX - panStartX);
      translateY = panStartTranslateY + (e.clientY - panStartY);
    }
  }

  function handleMouseUp() {
    isDraggingSlider = false;
    isPanning = false;
    if (container && $originalImageInfo) container.style.cursor = "grab";
  }

  function updateSliderPosition(e: MouseEvent) {
    if (!container) return;
    const rect = container.getBoundingClientRect();
    const x = e.clientX - rect.left;
    sliderPosition = Math.max(0, Math.min(100, (x / rect.width) * 100));
  }

  // ========================================================================
  // Zoom Logic (Centered on cursor)
  // ========================================================================

  function handleWheel(e: WheelEvent) {
    if (!container || !$originalImageInfo) return;
    e.preventDefault();

    const rect = container.getBoundingClientRect();
    const mouseX = e.clientX - rect.left;
    const mouseY = e.clientY - rect.top;

    const contentMouseX = (mouseX - translateX) / scale;
    const contentMouseY = (mouseY - translateY) / scale;

    const zoomIntensity = 0.1;
    const delta = e.deltaY < 0 ? 1 : -1;
    let newScale = scale + delta * zoomIntensity * scale;

    // Clamp Scale (0.1x a 50x para imágenes 4K)
    newScale = Math.max(0.1, Math.min(newScale, 50));

    translateX = mouseX - contentMouseX * newScale;
    translateY = mouseY - contentMouseY * newScale;

    scale = newScale;
  }

  // ========================================================================
  // View Control
  // ========================================================================

  function handleContextMenu(e: MouseEvent) {
    e.preventDefault();
    resetView();
  }

  function resetView() {
    scale = 1;
    translateX = 0;
    translateY = 0;
    sliderPosition = 50;
  }

  // ========================================================================
  // Touch Support
  // ========================================================================

  function handleTouchStart(e: TouchEvent) {
    if ((e.target as Element).closest(".slider-handle")) {
      isDraggingSlider = true;
    }
  }

  // ========================================================================
  // Empty State
  // ========================================================================

  function handleEmptyClick() {
    dispatch("openFile");
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.ctrlKey && e.key === "o") {
      e.preventDefault();
      dispatch("openFile");
    }
  }

  // Estado: tenemos imagen original cargada? (no esperar optimized)
  $: hasOriginal = $originalImageInfo !== null;
  // Estado: tenemos ambas imágenes? (para habilitar slider)
  $: hasOptimized = $optimizationResult !== null;
</script>

<svelte:window
  on:mousemove={handleMouseMove}
  on:mouseup={handleMouseUp}
  on:keydown={handleKeydown}
/>

<div
  class="compare-container"
  class:drag-over={isDragOver}
  class:has-image={hasOriginal}
  bind:this={container}
  on:mousedown={handleMouseDown}
  on:touchstart={handleTouchStart}
  on:wheel={handleWheel}
  on:contextmenu={handleContextMenu}
  role="slider"
  aria-valuenow={sliderPosition}
  aria-valuemin={0}
  aria-valuemax={100}
  tabindex="0"
>
  {#if hasOriginal}
    <!-- Fondo Checkerboard Estático -->
    <div class="checkerboard-bg"></div>

    <!-- Capa Original (visible a la IZQUIERDA del slider) -->
    <div
      class="image-layer original-layer"
      style="clip-path: inset(0 {100 - sliderPosition}% 0 0);"
    >
      <div class="transform-wrapper" style={transformStyle}>
        <canvas
          bind:this={canvasOriginal}
          class="image-canvas"
          style="width: {imageWidth}px; height: {imageHeight}px;"
        ></canvas>
      </div>
    </div>

    <!-- Capa Optimizada (visible a la DERECHA del slider) -->
    <div
      class="image-layer optimized-layer"
      style="clip-path: inset(0 0 0 {sliderPosition}%);"
    >
      {#if hasOptimized}
        <div class="transform-wrapper" style={transformStyle}>
          <canvas
            bind:this={canvasOptimized}
            class="image-canvas"
            style="width: {imageWidth}px; height: {imageHeight}px;"
          ></canvas>
        </div>
      {:else if $isProcessing}
        <!-- Mostrar indicador de carga mientras se procesa -->
        <div class="optimized-loading-overlay">
          <div class="processing-spinner"></div>
          <span class="optimized-loading-text">Optimizing...</span>
        </div>
      {/if}
    </div>

    <!-- UI Overlays -->
    <span class="label left-label">Original</span>
    <span class="label right-label">Optimized</span>

    <!-- FLOATING CLOSE BUTTON -->
    <button
      class="fab-close"
      title="Cerrar Imagen"
      on:click|stopPropagation={resetStores}
    >
      {@html generalIcons.iconClose}
    </button>

    <!-- Processing overlay (solo cuando ya hay imagen optimizada y se re-procesa) -->
    {#if $isProcessing && hasOptimized}
      <div class="processing-overlay">
        <div class="processing-spinner"></div>
        <span class="processing-text">Updating preview...</span>
      </div>
    {/if}

    <!-- Handle del slider -->
    <div class="slider-handle" style="left: {sliderPosition}%;">
      <div class="handle-line"></div>
      <div class="handle-grip">
        <svg
          width="20"
          height="20"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
        >
          <path d="M18 8L22 12L18 16"></path>
          <path d="M6 8L2 12L6 16"></path>
        </svg>
      </div>
    </div>

    <!-- Zoom indicator -->
    <div class="zoom-indicator">
      {Math.round(scale * 100)}%
    </div>
  {:else}
    <!-- Estado vacío -->
    <button
      class="empty-state"
      class:loading={$isLoading}
      on:click={handleEmptyClick}
      type="button"
      aria-label="Abrir imagen"
    >
      <div class="logo-container">
        <img src="/logo.svg" alt="Windoosh" class="logo" draggable="false" />
      </div>

      <div class="empty-content">
        {#if $isLoading}
          <div class="loading-spinner"></div>
          <p class="empty-title">Cargando imagen...</p>
        {:else if isDragOver}
          <p class="empty-title highlight">Suelta para abrir</p>
        {:else}
          <p class="empty-title">Arrastra una imagen aquí</p>
          <p class="empty-subtitle">o haz clic para explorar</p>
          <span class="shortcut-hint">
            <kbd>Ctrl</kbd> + <kbd>O</kbd>
          </span>
        {/if}
      </div>

      <div class="drop-zone-border"></div>
    </button>
  {/if}
</div>

<style>
  /* FLOATING ACTION BUTTON STYLES */
  .fab-close {
    position: absolute;
    top: 24px;
    right: 24px;
    width: 48px;
    height: 48px;
    border-radius: 50%;
    background: var(--primary); /* Squoosh Pink branding */
    color: white;
    border: none;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 24px;
    cursor: pointer;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
    z-index: 50;
    transition:
      transform 0.2s cubic-bezier(0.34, 1.56, 0.64, 1),
      background-color 0.2s;
  }

  .fab-close:hover {
    background: var(--primary-hover);
    transform: scale(1.1);
  }

  .fab-close:active {
    transform: scale(0.95);
  }

  .compare-container {
    position: relative;
    width: 100%;
    height: 100%;
    overflow: hidden;
    background: var(--bg-app);
    user-select: none;
  }

  .compare-container.has-image {
    cursor: grab;
  }

  .checkerboard-bg {
    position: absolute;
    inset: 0;
    background-image: linear-gradient(45deg, #2a2a2e 25%, transparent 25%),
      linear-gradient(-45deg, #2a2a2e 25%, transparent 25%),
      linear-gradient(45deg, transparent 75%, #2a2a2e 75%),
      linear-gradient(-45deg, transparent 75%, #2a2a2e 75%);
    background-size: 20px 20px;
    background-position:
      0 0,
      0 10px,
      10px -10px,
      -10px 0px;
    background-color: var(--bg-app);
    pointer-events: none;
    z-index: 0;
  }

  .image-layer {
    position: absolute;
    inset: 0;
    z-index: 1;
    pointer-events: none;
  }

  .original-layer {
    z-index: 1;
  }
  .optimized-layer {
    z-index: 2;
  }

  .transform-wrapper {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    will-change: transform;
    pointer-events: none;
  }

  /*
   * Canvas - RESOLUCIÓN COMPLETA
   * El canvas tiene width/height al tamaño REAL de la imagen
   * El zoom se hace via CSS transform, NO re-escalando píxeles
   */
  .image-canvas {
    pointer-events: none;
    box-shadow: 0 5px 20px rgba(0, 0, 0, 0.5);
    /* auto = smooth, pixelated = ver píxeles reales */
    image-rendering: auto;
  }

  .label {
    position: absolute;
    bottom: 24px;
    padding: 6px 14px;
    border-radius: 6px;
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 1px;
    backdrop-filter: blur(12px);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
    z-index: 20;
    pointer-events: none;
  }

  .left-label {
    left: 24px;
    background: rgba(255, 62, 108, 0.9);
    color: white;
  }

  .right-label {
    right: 24px;
    background: rgba(78, 205, 196, 0.9);
    color: #1a1a1a;
  }

  .slider-handle {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 40px;
    margin-left: -20px;
    z-index: 30;
    cursor: col-resize;
    display: flex;
    justify-content: center;
  }

  .handle-line {
    width: 2px;
    height: 100%;
    background: white;
    box-shadow: 0 0 10px rgba(0, 0, 0, 0.5);
  }

  .handle-grip {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: 44px;
    height: 44px;
    background: var(--bg-panel);
    border: 2px solid white;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: 0 8px 16px rgba(0, 0, 0, 0.4);
    color: white;
    cursor: col-resize;
  }

  .zoom-indicator {
    position: absolute;
    top: 16px;
    left: 50%;
    transform: translateX(-50%);
    padding: 6px 14px;
    border-radius: 6px;
    font-size: 12px;
    font-weight: 600;
    background: rgba(0, 0, 0, 0.7);
    color: white;
    backdrop-filter: blur(8px);
    z-index: 20;
    pointer-events: none;
  }

  .processing-overlay {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.3);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    z-index: 25;
    pointer-events: none;
  }

  .processing-text {
    color: white;
    font-size: 12px;
    font-weight: 500;
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.5);
  }

  /* Loading overlay para lado optimizado cuando no hay imagen aún */
  .optimized-loading-overlay {
    position: absolute;
    inset: 0;
    background: rgba(26, 26, 30, 0.9);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 16px;
    z-index: 5;
  }

  .optimized-loading-text {
    color: var(--text-muted);
    font-size: 14px;
    font-weight: 500;
  }

  .processing-spinner {
    width: 48px;
    height: 48px;
    border: 3px solid rgba(255, 255, 255, 0.2);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* Empty State */
  .empty-state {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
    background: var(--bg-app);
    border: none;
    cursor: pointer;
    transition: background-color 0.2s ease;
    font-family: inherit;
    padding: 0;
  }

  .empty-state:hover {
    background: rgba(255, 255, 255, 0.02);
  }

  .empty-state:focus {
    outline: none;
  }

  .empty-state:focus-visible .drop-zone-border {
    border-color: var(--accent);
  }

  .logo-container {
    margin-bottom: 24px;
  }

  .logo {
    width: 80px;
    height: 80px;
    animation: breathing 3s ease-in-out infinite;
    filter: drop-shadow(0 4px 12px rgba(0, 0, 0, 0.3));
  }

  @keyframes breathing {
    0%,
    100% {
      transform: scale(1);
      opacity: 0.85;
    }
    50% {
      transform: scale(1.05);
      opacity: 1;
    }
  }

  .empty-content {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    color: var(--text-muted);
  }

  .empty-title {
    font-size: 16px;
    font-weight: 500;
    margin: 0;
    color: var(--text-main);
    transition: color 0.2s ease;
  }

  .empty-title.highlight {
    color: var(--accent);
  }

  .empty-subtitle {
    font-size: 14px;
    margin: 0;
    opacity: 0.7;
  }

  .shortcut-hint {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 16px;
    font-size: 12px;
    color: var(--text-muted);
    opacity: 0.6;
  }

  kbd {
    display: inline-block;
    padding: 3px 8px;
    font-family: inherit;
    font-size: 11px;
    font-weight: 600;
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: 4px;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.2);
  }

  .drop-zone-border {
    position: absolute;
    inset: 24px;
    border: 2px dashed var(--border);
    border-radius: 16px;
    pointer-events: none;
    transition:
      border-color 0.2s ease,
      background-color 0.2s ease;
  }

  .empty-state:hover .drop-zone-border {
    border-color: var(--text-muted);
  }

  .compare-container.drag-over .drop-zone-border {
    border-color: var(--accent);
    background: rgba(78, 205, 196, 0.05);
  }

  .compare-container.drag-over .logo {
    animation: none;
    transform: scale(1.1);
    opacity: 1;
  }

  .empty-state.loading {
    cursor: wait;
  }

  .empty-state.loading .logo {
    animation: pulse-loading 1s ease-in-out infinite;
  }

  @keyframes pulse-loading {
    0%,
    100% {
      opacity: 0.5;
    }
    50% {
      opacity: 1;
    }
  }

  .loading-spinner {
    width: 24px;
    height: 24px;
    border: 2px solid var(--border);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
    margin-bottom: 8px;
  }
</style>
