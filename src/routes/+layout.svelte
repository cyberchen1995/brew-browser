<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { ui, watchSystemTheme } from "$lib/stores/ui.svelte";
  import { startEnvProbe } from "$lib/stores/env.svelte";
  import { activity } from "$lib/stores/activity.svelte";
  import { services } from "$lib/stores/services.svelte";
  import { settings } from "$lib/stores/settings.svelte";

  let { children } = $props();

  onMount(() => {
    ui.loadThemeFromStorage();
    // Settings (Phase 12b) — all read with enum/numeric validation so a
    // corrupt or hostile localStorage entry can't poison runtime state.
    ui.loadDefaultSectionFromStorage();
    ui.loadVibrancyMaterialFromStorage();
    ui.loadConfirmDestructiveFromStorage();
    ui.loadActivitySettingsFromStorage();
    activity.hydrate();
    // Phase 12d — hydrate the persisted settings.json into the renderer
    // so the Network section, the Catalog stale banner, and the cask
    // icon mode all read from one source of truth.
    void settings.load();
    // Prime the services list so the sidebar's "Services" badge can show a
    // count from first paint; the Services tab refreshes again on mount.
    void services.load();
    const unwatch = watchSystemTheme(() => ui.theme);
    const stopProbe = startEnvProbe();
    return () => {
      unwatch();
      stopProbe();
    };
  });
</script>

<!--
  Window dragging in Tauri 2 with titleBarStyle: "Overlay" is wired via the
  `data-tauri-drag-region` attribute on regular DOM elements (Sidebar brand
  area + each panel-head). Tauri's WebView handles click-vs-drag detection
  natively, so interactive children inside drag regions still receive their
  clicks. Avoids the fixed-overlay pattern (which intercepts scroll-wheel
  events at the top of the window).
-->

{@render children()}
