<!--
  ControlPanel.svelte
  Panel de controles "Dark Aesthetic" inspirado en Squoosh.
  Implementa secciones colapsables y controles estilizados.
-->
<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import {
    originalImageInfo,
    optimizationResult,
    encoderOptions,
    isLoading,
    isProcessing,
    originalSizeFormatted,
    optimizedSizeFormatted,
    savingsFormatted,
    type ImageInfo,
    type OptimizationResult,
  } from "$lib/stores/imageStore";
  import { slide } from "svelte/transition";
  import { generalIcons, panelSectionsIcons, toolsIcons } from "$lib/icons";

  let debounceTimer: ReturnType<typeof setTimeout>;

  // Estado local para UI (Resize)
  let resizeEnabled = false;
  let maintainAspectRatio = true;
  let aspectRatio = 1;
  let resizeWidth = 0;
  let resizeHeight = 0;
  let resizeMethod = "Lanczos3";
  let resizePreset = 100; // 100% default

  // Estado local para UI (Quantize)
  let quantizeEnabled = false;
  let quantizeColors = 256;
  let quantizeDither = 1.0;

  // Listener para drag & drop desde CompareSlider
  function handleDroppedFile(e: CustomEvent<string>) {
    loadImage(e.detail);
  }

  onMount(() => {
    window.addEventListener("load-dropped-file", handleDroppedFile as EventListener);
  });

  onDestroy(() => {
    window.removeEventListener("load-dropped-file", handleDroppedFile as EventListener);
    clearTimeout(debounceTimer);
  });

  // Reactividad para dimensiones iniciales
  $: if ($originalImageInfo && resizeWidth === 0 && !resizeEnabled) {
    resizeWidth = $originalImageInfo.width;
    resizeHeight = $originalImageInfo.height;
    aspectRatio = resizeWidth / resizeHeight;
  }

  // Handle Preset Change
  function handlePresetChange() {
    if (!$originalImageInfo) return;
    if (resizePreset === -1) return; // Custom

    const factor = resizePreset / 100;
    resizeWidth = Math.round($originalImageInfo.width * factor);
    resizeHeight = Math.round($originalImageInfo.height * factor);
    triggerProcess();
  }

  // Handlers de Resize
  function handleDimensionChange(dim: "width" | "height", value: number) {
    resizePreset = -1; // Seteamos a custom si el usuario edita manual

    if (dim === "width") {
      resizeWidth = value;
      if (maintainAspectRatio) {
        resizeHeight = Math.round(value / aspectRatio);
      }
    } else {
      resizeHeight = value;
      if (maintainAspectRatio) {
        resizeWidth = Math.round(value * aspectRatio);
      }
    }
    triggerProcess();
  }

  function toggleResize() {
    if (!resizeEnabled && $originalImageInfo) {
      // Reset to original when enabling
      resizeWidth = $originalImageInfo.width;
      resizeHeight = $originalImageInfo.height;
      resizePreset = 100;
      // Si estamos habilitando con valores = original, no procesar
      // (el resultado sería idéntico)
      return;
    }
    // Solo procesar si estamos deshabilitando (para volver a original sin resize)
    triggerProcess();
  }

  function toggleQuantize() {
    // Si estamos habilitando quantize con 256 colores, no hay cambio real
    // (256 colores = paleta completa, sin reducción)
    if (quantizeEnabled && quantizeColors === 256) {
      return;
    }
    triggerProcess();
  }

  function triggerProcess() {
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => processImage(), 400);
  }

  // Exponer para uso externo (desde +page.svelte)
  export async function handleOpenFile() {
    try {
      const selected = await open({
        multiple: false,
        filters: [
          {
            name: "Imágenes",
            extensions: ["png", "jpg", "jpeg", "webp", "gif", "bmp"],
          },
        ],
      });
      if (selected && typeof selected === "string") {
        await loadImage(selected);
      }
    } catch (err) {
      console.error("Error al abrir archivo:", err);
    }
  }

  // Exponer para uso externo
  export async function loadImage(path: string) {
    isLoading.set(true);
    try {
      const result = await invoke<ImageInfo>("load_image", { path });
      originalImageInfo.set(result);
      if (result) {
        resizeWidth = result.width;
        resizeHeight = result.height;
        aspectRatio = result.width / result.height;
      }
      await processImage();
    } catch (err) {
      console.error("Error al cargar imagen:", err);
    } finally {
      isLoading.set(false);
    }
  }

  async function processImage() {
    if (!$originalImageInfo) return;
    isProcessing.set(true);

    // Construir request
    const request: any = {
      encoder_name: $encoderOptions.encoder_name,
      options: $encoderOptions.options,
    };
    if (resizeEnabled) {
      request.resize = {
        width: resizeWidth,
        height: resizeHeight,
        filter: resizeMethod,
      };
    }
    if (quantizeEnabled) {
      request.quantize = {
        num_colors: quantizeColors,
        dither: quantizeDither,
      };
    }

    try {
      const result = await invoke<OptimizationResult>("process_image", {
        request,
      });
      optimizationResult.set(result);
    } catch (err) {
      console.error("Error al procesar imagen:", err);
    } finally {
      isProcessing.set(false);
    }
  }

  // Handlers
  function handleEncoderChange(e: Event) {
    const target = e.target as HTMLSelectElement;
    const name = target.value;
    // Squoosh Defaults
    if (name === "mozjpeg") {
      encoderOptions.set({ encoder_name: name, options: { quality: 75 } });
    } else if (name === "oxipng") {
      encoderOptions.set({
        encoder_name: name,
        options: { level: 2, interlace: false },
      });
    } else if (name === "webp") {
      encoderOptions.set({
        encoder_name: name,
        options: { quality: 75, lossless: false, method: 4 },
      });
    }
    triggerProcess();
  }

  function handleOptionChange(key: string, value: any) {
    const current = $encoderOptions;
    current.options[key] = value;
    encoderOptions.set(current);
    triggerProcess();
  }

  async function handleSave() {
    if (!$originalImageInfo || !$optimizationResult) return;
    const ext = $optimizationResult.extension;
    try {
      const selected = await save({
        filters: [
          {
            name: `Imagen ${$encoderOptions.encoder_name.toUpperCase()}`,
            extensions: [ext],
          },
        ],
        defaultPath: `optimized.${ext}`,
      });

      if (selected) {
        // Construir request save (mismo que process)
        const request: any = {
          encoder_name: $encoderOptions.encoder_name,
          options: $encoderOptions.options,
        };
        if (resizeEnabled) {
          request.resize = {
            width: resizeWidth,
            height: resizeHeight,
            filter: resizeMethod,
          };
        }
        if (quantizeEnabled) {
          request.quantize = {
            num_colors: quantizeColors,
            dither: quantizeDither,
          };
        }

        await invoke("save_image", { path: selected, request });
      }
    } catch (err) {
      console.error("Error al guardar:", err);
    }
  }
</script>

<aside class="control-panel">
  <!-- Header -->
  <div class="panel-header">
    <h1>Windoosh</h1>
    <button
      class="btn-icon"
      title="Abrir Imagen"
      on:click={handleOpenFile}
      disabled={$isLoading}
    >
      {#if $isLoading}
        <span class="loader"></span>
      {:else}
        {@html generalIcons.iconUpload}
      {/if}
    </button>
  </div>

  <div class="scroll-content">
    <!-- Sección: Edit (Resize) -->
    <section class="tool-section">
      <div class="section-header">
        <span>Edit</span>
        <span class="icon-svg">{@html panelSectionsIcons.iconEdit}</span>
      </div>

      <div class="tool-group">
        <div class="tool-title-row">
          <label for="resize-toggle">Resize</label>
          <label class="toggle-switch">
            <input
              type="checkbox"
              id="resize-toggle"
              bind:checked={resizeEnabled}
              on:change={toggleResize}
            />
            <span class="slider round"></span>
          </label>
        </div>

        {#if resizeEnabled}
          <div class="tool-content" transition:slide>
            <!-- Método -->
            <div class="control-group">
              <label for="resize-method" class="sub-label">Method</label>
              <select
                id="resize-method"
                bind:value={resizeMethod}
                on:change={triggerProcess}
              >
                <option value="Lanczos3">Lanczos3 (Sharp)</option>
                <option value="Mitchell">Mitchell (Smooth)</option>
                <option value="CatmullRom">CatmullRom</option>
                <option value="Triangle">Triangle (Fast)</option>
                <option value="Nearest">Nearest Neighbor (Pixel)</option>
              </select>

              <!-- Preset -->
              <div
                class="preset-row"
                style="margin-top: 10px; margin-bottom: 15px;"
              >
                <label for="resize-preset" class="sub-label">Preset</label>
                <select
                  id="resize-preset"
                  bind:value={resizePreset}
                  on:change={handlePresetChange}
                >
                  <option value={25}>25%</option>
                  <option value={33.33}>33.33%</option>
                  <option value={50}>50%</option>
                  <option value={100}>100%</option>
                  <option value={200}>200%</option>
                  <option value={300}>300%</option>
                  <option value={400}>400%</option>
                  <option value={-1}>Custom</option>
                </select>
              </div>
            </div>

            <div class="input-row">
              <div class="input-group">
                <label for="resize-width">Width</label>
                <input
                  id="resize-width"
                  type="number"
                  value={resizeWidth}
                  on:input={(e) =>
                    handleDimensionChange(
                      "width",
                      parseInt(e.currentTarget.value)
                    )}
                />
              </div>
              <!-- Aspect Ratio Lock Icon Button -->
              <div class="lock-btn-container">
                <button
                  class="btn-icon-small"
                  title="Toggle Aspect Ratio"
                  on:click={() => (maintainAspectRatio = !maintainAspectRatio)}
                  class:active={maintainAspectRatio}
                >
                  {#if maintainAspectRatio}
                    {@html toolsIcons.iconApectRatioLocked}
                  {:else}
                    {@html toolsIcons.iconAspectRatioUnlocked}
                  {/if}
                </button>
              </div>

              <div class="input-group">
                <label for="resize-height">Height</label>
                <input
                  id="resize-height"
                  type="number"
                  value={resizeHeight}
                  on:input={(e) =>
                    handleDimensionChange(
                      "height",
                      parseInt(e.currentTarget.value)
                    )}
                />
              </div>
            </div>

            <div class="control-group checkbox-row" style="margin-top: 10px;">
              <input
                type="checkbox"
                id="aspect-ratio"
                bind:checked={maintainAspectRatio}
              />
              <label for="aspect-ratio">Maintain Aspect Ratio</label>
            </div>
          </div>
        {/if}
      </div>

      <!-- Nuevo Grupo: Quantize -->
      <div
        class="tool-group"
        style="border-top: 1px solid var(--border); padding-top: 15px;"
      >
        <div class="tool-title-row">
          <label for="quantize-toggle">Reduce Palette</label>
          <label class="toggle-switch">
            <input
              type="checkbox"
              id="quantize-toggle"
              bind:checked={quantizeEnabled}
              on:change={toggleQuantize}
            />
            <span class="slider round"></span>
          </label>
        </div>

        {#if quantizeEnabled}
          <div class="tool-content" transition:slide>
            <div class="control-group">
              <div class="label-row">
                <label for="quantize-colors">Colors: {quantizeColors}</label>
              </div>
              <input
                id="quantize-colors"
                type="range"
                class="squoosh-slider"
                min="2"
                max="256"
                step="1"
                bind:value={quantizeColors}
                on:input={triggerProcess}
              />
            </div>
            <div class="control-group">
              <div class="label-row">
                <label for="quantize-dither"
                  >Dither: {(quantizeDither * 100).toFixed(0)}%</label
                >
              </div>
              <input
                id="quantize-dither"
                type="range"
                class="squoosh-slider"
                min="0"
                max="1"
                step="0.05"
                bind:value={quantizeDither}
                on:input={triggerProcess}
              />
            </div>
          </div>
        {/if}
      </div>
    </section>

    <!-- Sección: Compress -->
    <section class="tool-section compress-section">
      <div class="section-header">
        <span>Compress</span>
        <span class="icon-svg">{@html panelSectionsIcons.iconCompress}</span>
      </div>

      <div class="tool-content">
        <!-- Encoder Selector -->
        <div class="control-group">
          <select
            id="encoder"
            value={$encoderOptions.encoder_name}
            on:change={handleEncoderChange}
          >
            <option value="mozjpeg">MozJPEG</option>
            <option value="oxipng">OxiPNG</option>
            <option value="webp">WebP</option>
          </select>
        </div>

        <!-- Dynamic Options Form -->
        <div class="options-form">
          {#if $encoderOptions.encoder_name === "mozjpeg"}
            <div class="control-group">
              <div class="label-row">
                <label for="mozjpeg-quality">Quality</label>
                <span class="value">{$encoderOptions.options.quality}</span>
              </div>
              <input
                id="mozjpeg-quality"
                type="range"
                class="squoosh-slider"
                min="0"
                max="100"
                value={$encoderOptions.options.quality}
                on:input={(e) =>
                  handleOptionChange(
                    "quality",
                    parseInt(e.currentTarget.value)
                  )}
              />
              <!-- Visualización de barra de calidad -->
              <div
                class="quality-bar"
                style="--q: {$encoderOptions.options.quality}%"
              ></div>
            </div>
          {:else if $encoderOptions.encoder_name === "oxipng"}
            <div class="control-group">
              <div class="label-row">
                <label for="oxipng-effort">Effort</label>
                <span class="value">{$encoderOptions.options.level}</span>
              </div>
              <input
                id="oxipng-effort"
                type="range"
                class="squoosh-slider"
                min="0"
                max="6"
                value={$encoderOptions.options.level}
                on:input={(e) =>
                  handleOptionChange("level", parseInt(e.currentTarget.value))}
              />
              <div class="caption">Low setup time vs High compression</div>
            </div>
            <div class="control-group checkbox-row">
              <input
                type="checkbox"
                id="interlace"
                checked={!!$encoderOptions.options.interlace}
                on:change={(e) =>
                  handleOptionChange("interlace", e.currentTarget.checked)}
              />
              <label for="interlace">Interlace</label>
            </div>
          {:else if $encoderOptions.encoder_name === "webp"}
            <div class="control-group checkbox-row">
              <input
                type="checkbox"
                id="lossless"
                checked={!!$encoderOptions.options.lossless}
                on:change={(e) =>
                  handleOptionChange("lossless", e.currentTarget.checked)}
              />
              <label for="lossless">Lossless</label>
            </div>

            <div
              class="control-group"
              class:disabled={!!$encoderOptions.options.lossless}
            >
              <div class="label-row">
                <label for="webp-quality">Quality</label>
                <span class="value">{$encoderOptions.options.quality}</span>
              </div>
              <input
                id="webp-quality"
                type="range"
                class="squoosh-slider"
                min="0"
                max="100"
                disabled={!!$encoderOptions.options.lossless}
                value={$encoderOptions.options.quality}
                on:input={(e) =>
                  handleOptionChange(
                    "quality",
                    parseInt(e.currentTarget.value)
                  )}
              />
            </div>

            <div class="control-group">
              <div class="label-row">
                <label for="webp-effort">Effort</label>
                <span class="value">{$encoderOptions.options.method || 4}</span>
              </div>
              <input
                id="webp-effort"
                type="range"
                class="squoosh-slider"
                min="0"
                max="6"
                value={$encoderOptions.options.method || 4}
                on:input={(e) =>
                  handleOptionChange("method", parseInt(e.currentTarget.value))}
              />
            </div>
          {/if}
        </div>
      </div>
    </section>
  </div>

  <!-- Footer Actions -->
  {#if $optimizationResult}
    <div class="panel-footer" transition:slide>
      <div class="stats-card">
        <div class="stat-item">
          <span class="label">Original</span>
          <span class="val">{$originalSizeFormatted}</span>
        </div>
        <div class="stat-divider">↓</div>
        <div class="stat-item">
          <span class="label">{$optimizationResult.extension.toUpperCase()}</span>
          <span class="val highlight">{$optimizedSizeFormatted}</span>
        </div>
        <div
          class="stat-badge"
          class:negative={!$optimizationResult.savings_percent}
        >
          {$savingsFormatted}
        </div>
      </div>

      <button
        class="btn-primary btn-save"
        on:click={handleSave}
        disabled={$isProcessing}
      >
        {#if $isProcessing}Processing...{:else}Download{/if}
      </button>
    </div>
  {/if}
</aside>

<style>
  .control-panel {
    display: flex;
    flex-direction: column;
    width: 320px;
    height: 100%;
    background: var(--bg-panel);
    border-left: 1px solid var(--border);
    color: var(--text-main);
    user-select: none;
    font-size: 13px;
    box-shadow: -4px 0 20px rgba(0, 0, 0, 0.2);
  }

  .panel-header {
    padding: 16px 20px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid var(--border);
    background: var(--bg-app);
  }

  .panel-header h1 {
    margin: 0;
    font-size: 18px;
    font-weight: 700;
    letter-spacing: -0.5px;
    background: linear-gradient(90deg, #fff, #bbb);
    background-clip: text;
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
  }

  .btn-icon {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text-main);
    width: 32px;
    height: 32px;
    border-radius: 6px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: var(--transition-fast);
  }
  .btn-icon:hover {
    background: var(--border);
  }

  .scroll-content {
    flex: 1;
    overflow-y: auto;
    padding: 0;
    scrollbar-gutter: stable; /* Fixes layout jump on scrollbar appearance */
  }

  /* ... existing styles ... */

  .input-row {
    display: flex;
    align-items: flex-end; /* Alinea los inputs y el candado por la base */
    gap: 8px;              /* Espacio uniforme entre elementos */
    margin-bottom: 10px;
  }

  .input-group {
    flex: 1;               /* Esto hace que Width y Height midan exactamente lo mismo */
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .input-group label {
    font-size: 11px;
    color: var(--text-muted);
    font-weight: 600;
  }

  .input-group input {
    width: 100%;           /* Ocupa todo el espacio que le da el flex: 1 */
    box-sizing: border-box; /* Vital para que el padding no "infle" el input */
    background: var(--bg-input);
    border: 1px solid var(--border);
    color: var(--text-main);
    padding: 8px;
    border-radius: 6px;
    font-size: 13px;
    outline: none;
    text-align: center;
  }
  .input-group input:focus {
    border-color: var(--primary);
  }

  .lock-btn-container {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 33px; /* Altura aproximada del input para que el candado quede al centro */
    flex-shrink: 0; /* Evita que el botón se aplaste */
  }

  .btn-icon-small {
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;  /* Ancho fijo para el área del candado */
    height: 24px;
    padding: 0;   /* Elimina cualquier padding que mueva el icono */
    font-size: 18px;
    opacity: 0.6;
    transition: opacity 0.2s;
  }

  .tool-section {
    border-bottom: 1px solid var(--border);
  }

  .section-header {
    padding: 12px 20px;
    background: rgba(255, 255, 255, 0.03);
    font-weight: 600;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: var(--text-muted);
    display: flex;
    justify-content: space-between;
  }

  .tool-group {
    padding: 16px 20px;
  }

  .tool-title-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 10px;
  }

  .compress-section {
    background: rgba(0, 0, 0, 0.1);
  }
  .compress-section .tool-content {
    padding: 20px;
  }

  /* Inputs & Controls */
  select {
    width: 100%;
    background: var(--bg-input);
    border: 1px solid var(--border);
    color: var(--text-main);
    padding: 10px;
    border-radius: 6px;
    margin-bottom: 20px;
    font-size: 14px;
    outline: none;
    transition: border-color 0.2s;
  }
  select:focus {
    border-color: var(--primary);
  }

  .control-group {
    margin-bottom: 24px;
  }
  .control-group.disabled {
    opacity: 0.4;
    pointer-events: none;
  }

  .label-row {
    display: flex;
    justify-content: space-between;
    margin-bottom: 8px;
    font-size: 12px;
    font-weight: 500;
  }

  .squoosh-slider {
    appearance: none;
    -webkit-appearance: none;
    width: 100%;
    height: 4px;
    background: var(--border);
    border-radius: 2px;
    outline: none;
    cursor: pointer;
  }
  .squoosh-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 18px;
    height: 18px;
    background: var(--accent); /* Cyan accent */
    border-radius: 50%;
    border: 2px solid var(--bg-panel);
    box-shadow: 0 2px 5px rgba(0, 0, 0, 0.3);
    margin-top: -7px; /* Align center */
    transition: transform 0.1s;
  }
  .squoosh-slider::-webkit-slider-thumb:hover {
    transform: scale(1.1);
  }
  .squoosh-slider::-webkit-slider-runnable-track {
    width: 100%;
    height: 4px;
    cursor: pointer;
    background: var(--border);
    border-radius: 2px;
  }

  .checkbox-row {
    display: flex;
    align-items: center;
    gap: 10px;
    cursor: pointer;
  }
  .checkbox-row input {
    width: 16px;
    height: 16px;
    accent-color: var(--accent);
  }
  .checkbox-row label {
    cursor: pointer;
  }

  .caption {
    font-size: 11px;
    color: var(--text-muted);
    margin-top: 6px;
    font-style: italic;
  }

  /* Footer */
  .panel-footer {
    padding: 20px;
    background: var(--bg-panel);
    border-top: 1px solid var(--border);
    box-shadow: 0 -4px 20px rgba(0, 0, 0, 0.2);
  }

  .stats-card {
    display: flex;
    justify-content: space-between;
    align-items: center;
    background: var(--bg-input);
    padding: 10px 14px;
    border-radius: 8px;
    margin-bottom: 16px;
    font-size: 12px;
  }

  .stat-item {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .stat-item .label {
    color: var(--text-muted);
    font-size: 10px;
    text-transform: uppercase;
  }
  .stat-item .val {
    font-weight: 600;
  }
  .stat-item .val.highlight {
    color: var(--accent);
  }

  .stat-divider {
    color: var(--text-muted);
    opacity: 0.5;
  }

  .stat-badge {
    background: rgba(78, 205, 196, 0.15);
    color: var(--accent);
    padding: 4px 8px;
    border-radius: 4px;
    font-weight: 700;
  }
  .stat-badge.negative {
    background: rgba(255, 62, 108, 0.15);
    color: var(--primary);
  }

  .btn-save {
    width: 100%;
    background: var(--primary);
    color: white;
    border: none;
    padding: 14px;
    border-radius: 8px;
    font-weight: 600;
    font-size: 14px;
    cursor: pointer;
    display: flex;
    justify-content: center;
    align-items: center;
    transition: var(--transition-fast);
  }
  .btn-save:hover:not(:disabled) {
    background: var(--primary-hover);
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(255, 62, 108, 0.3);
  }
  .btn-save:disabled {
    background: var(--border);
    color: var(--text-muted);
    cursor: not-allowed;
  }

  .sub-label {
    display: block;
    font-size: 11px;
    color: var(--text-muted);
    margin-bottom: 6px;
    margin-top: 10px;
  }

  .icon-svg {
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 18px;
    color: var(--text-muted);
  }

  .btn-icon-small:hover {
    opacity: 1;
    color: var(--text-main);
  }
  .btn-icon-small.active {
    color: var(--accent);
    opacity: 1;
  }

  /* Slider */
  .toggle-switch {
    position: relative;
    display: inline-block;
    width: 36px;
    height: 20px;
  }
  .toggle-switch input {
    opacity: 0;
    width: 0;
    height: 0;
  }
  .slider {
    position: absolute;
    cursor: pointer;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background-color: var(--border);
    transition: 0.4s;
    border-radius: 20px;
  }
  .slider:before {
    position: absolute;
    content: "";
    height: 16px;
    width: 16px;
    left: 2px;
    bottom: 2px;
    background-color: white;
    transition: 0.4s;
    border-radius: 50%;
  }
  input:checked + .slider {
    background-color: var(--accent);
  }
  input:checked + .slider:before {
    transform: translateX(16px);
  }
  input:disabled + .slider {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
