<script lang="ts">
  import { originalImage, optimizedPreview, isLoading } from "$lib/stores/imageStore";
  import { createEventDispatcher, onMount, onDestroy } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";

  // Tipos para eventos de drag & drop en Tauri 2.0
  interface TauriDropPayload {
    type: "drop";
    paths: string[];
    position: { x: number; y: number };
  }

  const dispatch = createEventDispatcher<{ openFile: void }>();

  let sliderPosition = 50;
  let container: HTMLDivElement;

  // Transform State
  let scale = 1;
  let translateX = 0;
  let translateY = 0;
  let isDraggingSlider = false;
  let isPanning = false;
  let startX = 0;
  let startY = 0;

  // Drag & Drop State (Tauri native)
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

  onMount(async () => {
    try {
      // Tauri drag & drop events
      unlistenHover = await listen("tauri://drag-over", () => {
        isDragOver = true;
      });

      unlistenLeave = await listen("tauri://drag-leave", () => {
        isDragOver = false;
      });

      unlistenDrop = await listen<TauriDropPayload>("tauri://drag-drop", (event) => {
        isDragOver = false;
        const paths = event.payload.paths;
        if (paths && paths.length > 0) {
          const filePath = paths[0];
          if (isValidImagePath(filePath)) {
            window.dispatchEvent(new CustomEvent("load-dropped-file", { detail: filePath }));
          }
        }
      });
    } catch (e) {
      // En dev sin Tauri, ignorar
    }
  });

  onDestroy(() => {
    if (unlistenDrop) unlistenDrop();
    if (unlistenHover) unlistenHover();
    if (unlistenLeave) unlistenLeave();
  });

  // Computed transform style (shared by both layers)
  $: transformStyle = `transform: translate(${translateX}px, ${translateY}px) scale(${scale});`;

  // Slider Logic
  function handleMouseDown(e: MouseEvent) {
    if ((e.target as Element).closest(".slider-handle")) {
      isDraggingSlider = true;
      updateSliderPosition(e);
      e.preventDefault(); // Prevent text selection
    } else if ($originalImage && $optimizedPreview) {
      // Solo permitir pan cuando hay imagen
      isPanning = true;
      startX = e.clientX - translateX;
      startY = e.clientY - translateY;
      container.style.cursor = "grabbing";
    }
  }

  function handleMouseMove(e: MouseEvent) {
    if (isDraggingSlider) {
      updateSliderPosition(e);
    } else if (isPanning) {
      translateX = e.clientX - startX;
      translateY = e.clientY - startY;
    }
  }

  function handleMouseUp() {
    isDraggingSlider = false;
    isPanning = false;
    if (container && $originalImage) container.style.cursor = "grab";
  }

  function updateSliderPosition(e: MouseEvent) {
    if (!container) return;
    const rect = container.getBoundingClientRect();
    const x = e.clientX - rect.left;
    sliderPosition = Math.max(0, Math.min(100, (x / rect.width) * 100));
  }

  // Zoom Logic (Centered on cursor)
  function handleWheel(e: WheelEvent) {
    if (!container || !$originalImage) return;
    e.preventDefault();

    const rect = container.getBoundingClientRect();
    const mouseX = e.clientX - rect.left;
    const mouseY = e.clientY - rect.top;

    // Calculate Mouse Position relative to content (before zoom)
    const contentMouseX = (mouseX - translateX) / scale;
    const contentMouseY = (mouseY - translateY) / scale;

    const zoomIntensity = 0.1;
    const delta = e.deltaY < 0 ? 1 : -1;
    let newScale = scale + delta * zoomIntensity * scale;

    // Clamp Scale
    newScale = Math.max(0.1, Math.min(newScale, 20));

    // Calculate new Translate to keep contentMouse fixed
    translateX = mouseX - contentMouseX * newScale;
    translateY = mouseY - contentMouseY * newScale;

    scale = newScale;
  }

  // Restore/Reset View
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

  // Touch Support (Basic Slider only for now, can be expanded)
  function handleTouchStart(e: TouchEvent) {
    if ((e.target as Element).closest(".slider-handle")) {
      isDraggingSlider = true;
    }
  }

  // Click to open (empty state)
  function handleEmptyClick() {
    dispatch("openFile");
  }

  // Keyboard shortcut
  function handleKeydown(e: KeyboardEvent) {
    if (e.ctrlKey && e.key === "o") {
      e.preventDefault();
      dispatch("openFile");
    }
  }
</script>

<svelte:window on:mousemove={handleMouseMove} on:mouseup={handleMouseUp} on:keydown={handleKeydown} />

<div
  class="compare-container"
  class:drag-over={isDragOver}
  class:has-image={$originalImage && $optimizedPreview}
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
  {#if $originalImage && $optimizedPreview}
    <!-- Fondo Checkerboard Estático (no se mueve con pan/zoom) -->
    <div class="checkerboard-bg"></div>

    <!-- Capa Original (visible a la IZQUIERDA del slider) -->
    <div 
      class="image-layer original-layer" 
      style="clip-path: inset(0 {100 - sliderPosition}% 0 0);"
    >
      <div class="transform-wrapper" style={transformStyle}>
        <img
          src={$originalImage.preview_base64}
          alt="Original"
          draggable="false"
        />
      </div>
    </div>

    <!-- Capa Optimizada (visible a la DERECHA del slider) -->
    <div 
      class="image-layer optimized-layer" 
      style="clip-path: inset(0 0 0 {sliderPosition}%);"
    >
      <div class="transform-wrapper" style={transformStyle}>
        <img
          src={$optimizedPreview.preview_base64}
          alt="Optimizada"
          draggable="false"
        />
      </div>
    </div>

    <!-- UI Overlays -->
    <span class="label left-label">Original</span>
    <span class="label right-label">Optimized</span>

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
  {:else}
    <!-- Estado vacío mejorado (D+E+F+G) -->
    <button 
      class="empty-state" 
      class:loading={$isLoading}
      on:click={handleEmptyClick}
      type="button"
      aria-label="Abrir imagen"
    >
      <!-- Logo con animación breathing (E) -->
      <div class="logo-container">
        <img src="/logo.svg" alt="Windoosh" class="logo" draggable="false" />
      </div>

      <!-- Mensaje principal -->
      <div class="empty-content">
        {#if $isLoading}
          <div class="loading-spinner"></div>
          <p class="empty-title">Cargando imagen...</p>
        {:else if isDragOver}
          <p class="empty-title highlight">Suelta para abrir</p>
        {:else}
          <!-- Botón grande (G) -->
          <p class="empty-title">Arrastra una imagen aquí</p>
          <p class="empty-subtitle">o haz clic para explorar</p>
          <!-- Shortcut (F) -->
          <span class="shortcut-hint">
            <kbd>Ctrl</kbd> + <kbd>O</kbd>
          </span>
        {/if}
      </div>

      <!-- Drop zone visual feedback -->
      <div class="drop-zone-border"></div>
    </button>
  {/if}
</div>

<style>
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

  /* Fondo Checkerboard Estático - NO se mueve con pan/zoom */
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

  /* Capas de imagen - el clip-path se aplica AQUÍ (nivel viewport) */
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

  /* Transform wrapper - contiene la imagen que se transforma */
  .transform-wrapper {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    will-change: transform;
    transform-origin: 0 0;
    pointer-events: none;
  }

  .transform-wrapper img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    pointer-events: none;
    box-shadow: 0 5px 20px rgba(0, 0, 0, 0.5);
  }

  /* Labels */
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

  /* Slider Handle */
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

  /* ========================================
     EMPTY STATE MEJORADO (D+E+F+G)
     ======================================== */
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

  /* Logo con animación breathing (E) */
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
    0%, 100% {
      transform: scale(1);
      opacity: 0.85;
    }
    50% {
      transform: scale(1.05);
      opacity: 1;
    }
  }

  /* Contenido del empty state */
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

  /* Shortcut hint (F) */
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

  /* Drop zone border (D) */
  .drop-zone-border {
    position: absolute;
    inset: 24px;
    border: 2px dashed var(--border);
    border-radius: 16px;
    pointer-events: none;
    transition: border-color 0.2s ease, background-color 0.2s ease;
  }

  .empty-state:hover .drop-zone-border {
    border-color: var(--text-muted);
  }

  /* Drag Over State */
  .compare-container.drag-over .drop-zone-border {
    border-color: var(--accent);
    background: rgba(78, 205, 196, 0.05);
  }

  .compare-container.drag-over .logo {
    animation: none;
    transform: scale(1.1);
    opacity: 1;
  }

  /* Loading State */
  .empty-state.loading {
    cursor: wait;
  }

  .empty-state.loading .logo {
    animation: pulse-loading 1s ease-in-out infinite;
  }

  @keyframes pulse-loading {
    0%, 100% {
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

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
