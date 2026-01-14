import { writable, get } from "svelte/store";
import { invoke } from "@tauri-apps/api/core";

export interface OptimizePreset {
    id: string;
    name: string;
    encoder: {
        name: "mozjpeg" | "oxipng" | "webp";
        options: any;
    };
    resize?: {
        enabled: boolean;
        width?: number;
        height?: number;
        method?: string; // Lancaster3, etc.
    };
    quantize?: {
        enabled: boolean;
        colors: number;
        dither: number;
    };
}

export interface AppSettings {
    contextMenuEnabled: boolean;
    presets: OptimizePreset[];
}

const defaultPresets: OptimizePreset[] = [
    {
        id: "default-webp",
        name: "WebP Optimized",
        encoder: { name: "webp", options: { quality: 80, lossless: false, method: 4 } }
    },
    {
        id: "default-jpeg",
        name: "JPEG Web",
        encoder: { name: "mozjpeg", options: { quality: 75 } }
    }
];

const defaultSettings: AppSettings = {
    contextMenuEnabled: false,
    presets: defaultPresets
};

export const settingsStore = writable<AppSettings>(defaultSettings);

const SETTINGS_KEY = "windoosh_settings_v3";

export function loadSettings() {
    const stored = localStorage.getItem(SETTINGS_KEY);
    if (stored) {
        try {
            const parsed = JSON.parse(stored);
            settingsStore.set({ ...defaultSettings, ...parsed });
        } catch (e) {
            console.error("Error loading settings", e);
        }
    }
}

export function saveSettings(settings: AppSettings) {
    settingsStore.set(settings);
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings));
    // Auto-update registry if enabled
    if (settings.contextMenuEnabled) {
        updateRegistry(settings.presets);
    }
}

async function updateRegistry(presets: OptimizePreset[]) {
    try {
        const items = presets.map(p => ({ id: p.id, name: p.name }));
        await invoke("update_context_menu_items", { items });
    } catch (e) {
        console.error("Error updating context menu registry", e);
    }
}

export async function toggleContextMenu(enable: boolean) {
    settingsStore.update(s => {
        const next = { ...s, contextMenuEnabled: enable };
        localStorage.setItem(SETTINGS_KEY, JSON.stringify(next));
        return next;
    });
    
    try {
        await invoke("toggle_context_menu", { enable });
         if (enable) {
             const current = get(settingsStore).presets;
             await updateRegistry(current);
         }
    } catch (e) {
        console.error("Error toggling backend context menu", e);
    }
}

export async function syncContextMenuState() {
     try {
        const isActive = await invoke<boolean>("get_context_menu_state");
        settingsStore.update(s => ({ ...s, contextMenuEnabled: isActive }));
    } catch (e) {
        console.error("Error syncing context menu state", e);
    }
}

export function generateId(): string {
    return Math.random().toString(36).substring(2, 15) + Math.random().toString(36).substring(2, 15);
}
