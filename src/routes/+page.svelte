<!--
  +page.svelte
  Página principal de Windoosh - Optimizador de imágenes
-->
<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import CompareSlider from "$lib/components/CompareSlider.svelte";
  import ControlPanel from "$lib/components/ControlPanel.svelte";
  import { originalImageInfo } from "$lib/stores/imageStore";

  let controlPanel: ControlPanel;
  let unlistenFn: UnlistenFn | null = null;

  async function handlePaste(event: ClipboardEvent) {
    // Si ya hay una imagen cargada, no reemplazarla (como solicitado)
    // "al tener la aplicación sin ningúna imagen actual"
    if ($originalImageInfo) return;

    const items = event.clipboardData?.items;
    if (!items) return;

    for (const item of items) {
      if (item.type.startsWith("image/")) {
        event.preventDefault();
        const file = item.getAsFile();
        if (file && controlPanel) {
          const buffer = await file.arrayBuffer();
          const bytes = new Uint8Array(buffer);
          await controlPanel.loadFromClipboard(bytes);
          return; // Solo procesar la primera imagen encontrada
        }
      }
    }
  }

  onMount(() => {
    // Ocultar splash screen
    const splash = document.getElementById("splash");
    if (splash) {
      splash.classList.add("hidden");
      setTimeout(() => splash.remove(), 300);
    }

    // Mostrar ventana (async pero no bloqueante)
    getCurrentWindow()
      .show()
      .catch(() => {
        // En dev mode sin Tauri, ignorar
      });

    // Escuchar evento de archivo abierto desde menú contextual
    listen<string>("open-file-from-args", (event) => {
      if (controlPanel && event.payload) {
        controlPanel.loadImage(event.payload);
      }
    }).then((fn) => {
      unlistenFn = fn;
    });
  });

  onDestroy(() => {
    if (unlistenFn) unlistenFn();
  });

  // Exponer función para abrir archivo desde CompareSlider
  function handleOpenFile() {
    if (controlPanel) {
      controlPanel.handleOpenFile();
    }
  }
</script>

<svelte:window on:paste={handlePaste} />

<div class="app">
  <main class="workspace">
    <CompareSlider on:openFile={handleOpenFile} />
  </main>
  <ControlPanel bind:this={controlPanel} />
</div>

<style>
  .app {
    display: flex;
    height: 100vh;
    width: 100vw;
  }

  .workspace {
    flex: 1;
    /* padding: 24px;  Removido para full immersion */
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(
      --bg-app
    ); /* Checkerboard ya está en CompareSlider si vacio? No, en empty state */
  }
</style>
