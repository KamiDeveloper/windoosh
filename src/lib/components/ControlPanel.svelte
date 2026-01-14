<!--
  ControlPanel.svelte
  Panel de controles "Dark Aesthetic" inspirado en Squoosh.
  Implementa secciones colapsables y controles estilizados.
-->
<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { get } from "svelte/store";
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
    droppedFile,
    type ImageInfo,
    type OptimizationResult,
    type OptimizationRequest,
    type EncoderOptions,
    resetStores,
  } from "$lib/stores/imageStore";
  import { listen } from "@tauri-apps/api/event";
  import {
    settingsStore,
    loadSettings,
    saveSettings,
    toggleContextMenu,
    syncContextMenuState,
    generateId, // Assuming this is exported now
    type AppSettings,
    type OptimizePreset,
  } from "$lib/stores/settingsStore";
  import {
    generalIcons,
    panelSectionsIcons,
    toolsIcons,
    specificControlsIcons,
  } from "$lib/icons"; // Added specificControlsIcons for modal
  import { slide } from "svelte/transition";

  let debounceTimer: ReturnType<typeof setTimeout>;
  let showSettings = false;

  // Preset Create/Edit State
  let editingPresetId: string | null = null;
  let presetForm = {
    name: "",
    encoder: "webp",
    quality: 80,
    effort: 4,
    resize: false,
    width: 0,
    quantize: false,
    colors: 128,
  };

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

  // Listener para drag & drop desde CompareSlider via Store
  $: if ($droppedFile) {
    loadImage($droppedFile);
    droppedFile.set(null); // Reset after consumption
  }

  onMount(() => {
    let unlisten: (() => void) | undefined;
    let unlistenPreset: (() => void) | undefined;

    const init = async () => {
      loadSettings();
      await syncContextMenuState();

      // 1. Listen for generic quick optimize (Legacy support)
      unlisten = await listen<string>("quick-optimize", async (event) => {
        // Apply first preset or default
        const presets = get(settingsStore).presets;
        if (presets.length > 0) {
          applyPreset(presets[0], event.payload);
        }
      });

      // 2. Listen for specific presets
      unlistenPreset = await listen<[string, string]>(
        "preset-optimize",
        async (event) => {
          const [presetId, filePath] = event.payload;
          const presets = get(settingsStore).presets;
          const preset = presets.find((p) => p.id === presetId);
          if (preset && filePath) {
            await applyPreset(preset, filePath);
          }
        }
      );
    };

    init();

    return () => {
      if (unlisten) unlisten();
      if (unlistenPreset) unlistenPreset();
    };
  });

  async function applyPreset(preset: any, filePath: string) {
    await loadImage(filePath);
    // Set Encoder
    encoderOptions.set({
      encoder_name: preset.encoder.name,
      options: preset.encoder.options,
    });
    // Set Resize if enabled
    if (preset.resize && preset.resize.enabled) {
      resizeEnabled = true;
      // Logic for width/height/preset?
      // For now let's just assume explicit options or default
      resizeMethod = preset.resize.method || "Lanczos3";

      if (preset.resize.width) {
        resizeWidth = preset.resize.width;
        // Auto calc height if we had aspect ratio...
        // But valid image is loaded now, so we can calculate
      }
    } else {
      resizeEnabled = false;
    }

    // Set Quantize if enabled
    if (preset.quantize && preset.quantize.enabled) {
      quantizeEnabled = true;
      quantizeColors = preset.quantize.colors;
      quantizeDither = preset.quantize.dither;
    } else {
      quantizeEnabled = false;
    }

    triggerProcess();
  }

  onDestroy(() => {
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
  export async function loadFromClipboard(bytes: Uint8Array) {
    isLoading.set(true);
    try {
      // Tauri maneja Uint8Array -> Vec<u8> automáticamente
      const result = await invoke<ImageInfo>("load_image_from_bytes", {
        bytes,
      });
      originalImageInfo.set(result);
      if (result) {
        resizeWidth = result.width;
        resizeHeight = result.height;
        aspectRatio = result.width / result.height;
      }
      await processImage();
    } catch (err) {
      console.error("Error al cargar imagen del portapapeles:", err);
    } finally {
      isLoading.set(false);
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

    // Construir request con tipos estrictos
    const request: OptimizationRequest = {
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
      // Smart Filename: original_name + "_optimized"
      const originalName = $originalImageInfo.name;
      const baseName =
        originalName.lastIndexOf(".") !== -1
          ? originalName.substring(0, originalName.lastIndexOf("."))
          : originalName;

      const defaultName = `${baseName}_optimized.${ext}`;

      const selected = await save({
        filters: [
          {
            name: `Imagen ${$encoderOptions.encoder_name.toUpperCase()}`,
            extensions: [ext],
          },
        ],
        defaultPath: defaultName,
      });

      if (selected) {
        // Construir request save (mismo que process)
        const request: OptimizationRequest = {
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

  async function handleToggleContextMenu(e: Event) {
    const checked = (e.target as HTMLInputElement).checked;
    try {
      await toggleContextMenu(checked);
    } catch (err) {
      console.error("Failed to toggle context menu:", err);
      await syncContextMenuState();
    }
  }

  // --- Preset Logic ---
  function createNewPreset() {
    presetForm = {
      name: "New Preset",
      encoder: "webp",
      quality: 80,
      effort: 4,
      resize: false,
      width: 800,
      quantize: false,
      colors: 128,
    };
    editingPresetId = "NEW";
  }

  function editPreset(preset: OptimizePreset) {
    editingPresetId = preset.id;
    // Map schema to form
    const isJpeg = preset.encoder.name === "mozjpeg";
    const isWebp = preset.encoder.name === "webp";
    const isPng = preset.encoder.name === "oxipng";

    presetForm = {
      name: preset.name,
      encoder: preset.encoder.name,
      quality: (preset.encoder.options as any).quality || 80,
      effort:
        (preset.encoder.options as any).level ||
        (preset.encoder.options as any).method ||
        4,
      resize: preset.resize?.enabled || false,
      width: preset.resize?.width || 800,
      quantize: preset.quantize?.enabled || false,
      colors: preset.quantize?.colors || 128,
    };
  }

  function deletePreset(id: string) {
    const s = get(settingsStore);
    const newPresets = s.presets.filter((p) => p.id !== id);
    saveSettings({ ...s, presets: newPresets });
  }

  function savePreset() {
    const s = get(settingsStore);
    let newPresets = [...s.presets];

    const newPreset: OptimizePreset = {
      id: editingPresetId === "NEW" ? generateId() : editingPresetId!,
      name: presetForm.name,
      encoder: {
        name: presetForm.encoder as any,
        options: {},
      },
      resize: {
        enabled: presetForm.resize,
        width: presetForm.width,
        method: "Lanczos3",
      },
      quantize: {
        enabled: presetForm.quantize,
        colors: presetForm.quantize ? presetForm.colors : 256,
        dither: 1.0,
      },
    };

    // Set encoder options
    if (presetForm.encoder === "mozjpeg") {
      newPreset.encoder.options = { quality: presetForm.quality };
    } else if (presetForm.encoder === "webp") {
      newPreset.encoder.options = {
        quality: presetForm.quality,
        lossless: false,
        method: presetForm.effort,
      };
    } else if (presetForm.encoder === "oxipng") {
      newPreset.encoder.options = {
        level: presetForm.effort,
        interlace: false,
      };
    }

    if (editingPresetId === "NEW") {
      newPresets.push(newPreset);
    } else {
      const idx = newPresets.findIndex((p) => p.id === editingPresetId);
      if (idx !== -1) newPresets[idx] = newPreset;
    }

    saveSettings({ ...s, presets: newPresets });
    editingPresetId = null;
  }
</script>

<aside class="control-panel">
  <!-- Header -->
  <div class="panel-header">
    <h1>Windoosh</h1>
    <div class="header-actions">
      <button
        class="btn-icon"
        title="Settings"
        on:click={() => (showSettings = true)}
      >
        {@html panelSectionsIcons.iconSettings}
      </button>

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
  </div>

  <!-- Settings Modal -->
  {#if showSettings}
    <div
      class="modal-backdrop"
      role="button"
      tabindex="0"
      on:click|self={() => (showSettings = false)}
      on:keydown={(e) => e.key === "Escape" && (showSettings = false)}
    >
      <div class="modal-content" transition:slide={{ duration: 200 }}>
        <div class="modal-header">
          <h2>Settings</h2>
          <button class="btn-icon" on:click={() => (showSettings = false)}>
            {@html generalIcons.iconClose}
          </button>
        </div>

        <div class="modal-body">
          <!-- Windows Integration -->
          <section class="settings-section">
            <h3>System Integration</h3>
            <div class="control-group checkbox-row">
              <input
                type="checkbox"
                id="ctx-menu"
                checked={$settingsStore.contextMenuEnabled}
                on:change={handleToggleContextMenu}
              />
              <label for="ctx-menu">Show in Windows Context Menu</label>
            </div>
          </section>

          <!-- Quick Optimize Defaults -->
          <section class="settings-section">
            <h3>Presets Manager</h3>
            <p class="hint">
              Define the presets available in the "Quick Optimize" submenu.
            </p>

            {#if !editingPresetId}
              <!-- List View -->
              <div class="presets-list">
                {#each $settingsStore.presets as preset}
                  <div class="preset-item">
                    <div class="preset-info">
                      <span class="preset-name">{preset.name}</span>
                      <span class="preset-details">{preset.encoder.name}</span>
                    </div>
                    <div class="preset-actions">
                      <button
                        class="btn-icon-small"
                        on:click={() => editPreset(preset)}>✎</button
                      >
                      <button
                        class="btn-icon-small"
                        on:click={() => deletePreset(preset.id)}>🗑</button
                      >
                    </div>
                  </div>
                {/each}
                <button
                  class="btn-primary"
                  style="margin-top: 10px; width: 100%;"
                  on:click={createNewPreset}>+ Add New Preset</button
                >
              </div>
            {:else}
              <!-- Edit View -->
              <div class="preset-edit-form" transition:slide>
                <div class="control-group">
                  <label for="preset-name">Preset Name</label>
                  <input
                    type="text"
                    bind:value={presetForm.name}
                    placeholder="My Preset"
                    class="settings-input"
                  />
                </div>

                <div class="control-group">
                  <label for="format">Format</label>
                  <select bind:value={presetForm.encoder}>
                    <option value="mozjpeg">MozJPEG</option>
                    <option value="oxipng">OxiPNG</option>
                    <option value="webp">WebP</option>
                  </select>
                </div>

                {#if presetForm.encoder === "mozjpeg" || presetForm.encoder === "webp"}
                  <div class="control-group">
                    <label for="quality">Quality ({presetForm.quality})</label>
                    <input
                      type="range"
                      min="0"
                      max="100"
                      bind:value={presetForm.quality}
                      class="squoosh-slider"
                    />
                    <div
                      class="quality-bar"
                      style="--q: {presetForm.quality}%"
                    ></div>
                  </div>
                {/if}

                {#if presetForm.encoder === "oxipng" || presetForm.encoder === "webp"}
                  <div class="control-group">
                    <label for="effort">Effort ({presetForm.effort})</label>
                    <input
                      type="range"
                      min="0"
                      max="6"
                      bind:value={presetForm.effort}
                      class="squoosh-slider"
                    />
                  </div>
                {/if}

                <!-- Resize Option -->
                <div class="control-group checkbox-row">
                  <input
                    type="checkbox"
                    id="p-resize"
                    bind:checked={presetForm.resize}
                  />
                  <label for="p-resize">Resize (Width Only)</label>
                </div>
                {#if presetForm.resize}
                  <div class="control-group">
                    <input
                      type="number"
                      bind:value={presetForm.width}
                      placeholder="Width (px)"
                      class="settings-input"
                    />
                    <span class="hint">Height calculated automatically</span>
                  </div>
                {/if}

                <!-- Quantize Option -->
                <div class="control-group checkbox-row">
                  <input
                    type="checkbox"
                    id="p-quant"
                    bind:checked={presetForm.quantize}
                  />
                  <label for="p-quant">Reduce Palette</label>
                </div>
                {#if presetForm.quantize}
                  <div class="control-group">
                    <label for="colors">Colors ({presetForm.colors})</label>
                    <input
                      type="range"
                      min="2"
                      max="256"
                      bind:value={presetForm.colors}
                      class="squoosh-slider"
                    />
                  </div>
                {/if}

                <div
                  class="form-actions"
                  style="display: flex; gap: 10px; margin-top: 20px;"
                >
                  <button class="btn-save" on:click={savePreset}>Save</button>
                  <button
                    class="btn-icon"
                    style="flex:1"
                    on:click={() => (editingPresetId = null)}>Cancel</button
                  >
                </div>
              </div>
            {/if}
          </section>
        </div>
      </div>
    </div>
  {/if}

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

            <!-- WebP Effort slider removed because backend crate 'webp' v0.3 does not support it exposed in public API yet -->
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
          <span class="label"
            >{$optimizationResult.extension.toUpperCase()}</span
          >
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

  .header-actions {
    display: flex;
    gap: 8px;
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
    gap: 8px; /* Espacio uniforme entre elementos */
    margin-bottom: 10px;
  }

  .input-group {
    flex: 1; /* Esto hace que Width y Height midan exactamente lo mismo */
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
    width: 100%; /* Ocupa todo el espacio que le da el flex: 1 */
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
    width: 24px; /* Ancho fijo para el área del candado */
    height: 24px;
    padding: 0; /* Elimina cualquier padding que mueva el icono */
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
  /* Modal Styles */
  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 9999;
    background: rgba(0, 0, 0, 0.7);
    backdrop-filter: blur(5px);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .modal-content {
    background: #18181b; /* Zinc-900 hardcoded fallback or var(--bg-panel) if opaque */
    border: 1px solid var(--border);
    border-radius: 12px;
    width: 340px;
    max-width: 90vw;
    box-shadow: 0 20px 50px rgba(0, 0, 0, 0.5);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    color: var(--text-main);
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px;
    border-bottom: 1px solid var(--border);
    background: rgba(255, 255, 255, 0.03);
  }

  .modal-header h2 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
    color: var(--text-main);
  }

  .modal-body {
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 24px;
  }

  .settings-section h3 {
    margin: 0 0 12px 0;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: var(--text-muted);
    border-bottom: 1px solid var(--border);
    padding-bottom: 6px;
  }

  .hint {
    font-size: 11px;
    color: var(--text-muted);
    margin-top: -8px;
    margin-bottom: 16px;
    line-height: 1.4;
  }

  /* Fix for close button in modal */
  .modal-header .btn-icon {
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border-radius: 6px;
    font-size: 20px;
    transition: all 0.2s;
  }
  .modal-header .btn-icon:hover {
    background: rgba(255, 255, 255, 0.1);
    color: white;
  }
  /* Preset Styles */
  .presets-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .preset-item {
    background: var(--bg-input);
    border: 1px solid var(--border);
    padding: 10px;
    border-radius: 6px;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .preset-info {
    display: flex;
    flex-direction: column;
  }
  .preset-name {
    font-weight: 600;
    font-size: 13px;
  }
  .preset-details {
    font-size: 11px;
    color: var(--text-muted);
    text-transform: uppercase;
  }
  .preset-actions {
    display: flex;
    gap: 4px;
  }
  .settings-input {
    background: var(--bg-input);
    border: 1px solid var(--border);
    color: var(--text-main);
    padding: 8px;
    border-radius: 6px;
    font-size: 13px;
    outline: none;
    width: 100%;
    box-sizing: border-box;
  }
  .settings-input:focus {
    border-color: var(--primary);
  }
</style>
