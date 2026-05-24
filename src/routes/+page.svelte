<script lang="ts">
  import { onMount } from "svelte";

  import Sidebar from "$lib/components/Sidebar.svelte";
  import Dashboard from "$lib/components/Dashboard.svelte";
  import Library from "$lib/components/Library.svelte";
  import Discover from "$lib/components/Discover.svelte";
  import Trending from "$lib/components/Trending.svelte";
  import Snapshots from "$lib/components/Snapshots.svelte";
  import Services from "$lib/components/Services.svelte";
  import ActivityHistory from "$lib/components/ActivityHistory.svelte";
  import PackageDetail from "$lib/components/PackageDetail.svelte";
  import ResizeHandle from "$lib/components/ResizeHandle.svelte";
  import ActivityDrawer from "$lib/components/ActivityDrawer.svelte";
  import CommandPalette from "$lib/components/CommandPalette.svelte";
  import Settings from "$lib/components/Settings.svelte";
  import DeviceFlowModal from "$lib/components/DeviceFlowModal.svelte";
  import Toast from "$lib/components/Toast.svelte";

  import { ui } from "$lib/stores/ui.svelte";
  import { DETAIL_PANE_MIN_WIDTH, DETAIL_PANE_DEFAULT_WIDTH, clampDetailPaneWidth } from "$lib/stores/ui.svelte";
  import { packages } from "$lib/stores/packages.svelte";
  import { brewfiles } from "$lib/stores/brewfiles.svelte";
  import { trending } from "$lib/stores/trending.svelte";
  import { services } from "$lib/stores/services.svelte";
  import { search } from "$lib/stores/search.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import type { SidebarSection, ThemePreference } from "$lib/types";

  const themeLabel: Record<ThemePreference, string> = {
    light: "Light",
    dark: "Dark",
    system: "System",
  };

  function isTextInput(el: EventTarget | null): boolean {
    if (!(el instanceof HTMLElement)) return false;
    return el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable;
  }

  function onKeydown(e: KeyboardEvent) {
    const meta = e.metaKey || e.ctrlKey;

    // Cmd+K: open palette (always)
    if (meta && e.key.toLowerCase() === "k") {
      e.preventDefault();
      ui.openPalette();
      return;
    }

    // Cmd+, : open Settings (macOS preferences convention)
    if (meta && e.key === ",") {
      e.preventDefault();
      ui.openSettings();
      return;
    }

    // Cmd+Shift+L: cycle theme (must precede Cmd+L)
    if (meta && e.shiftKey && e.key.toLowerCase() === "l") {
      e.preventDefault();
      const order = ["light", "dark", "system"] as const;
      const next = order[(order.indexOf(ui.theme) + 1) % order.length];
      ui.setTheme(next);
      toast.info(`Theme: ${themeLabel[next]}`);
      return;
    }

    // Cmd+L (no shift): toggle drawer
    if (meta && !e.shiftKey && e.key.toLowerCase() === "l") {
      e.preventDefault();
      ui.toggleDrawer();
      return;
    }

    // Cmd+0..6: section nav (0 = dashboard / home)
    if (meta && ["0","1","2","3","4","5","6"].includes(e.key)) {
      e.preventDefault();
      const map: Record<string, SidebarSection> = {
        "0": "dashboard",
        "1": "library",
        "2": "discover",
        "3": "trending",
        "4": "snapshots",
        "5": "services",
        "6": "activity",
      };
      ui.setSection(map[e.key]);
      return;
    }

    // Cmd+R: refresh current view
    if (meta && e.key.toLowerCase() === "r") {
      e.preventDefault();
      switch (ui.section) {
        case "dashboard": packages.load(true); break;
        case "library":   packages.load(true); break;
        case "trending":  trending.load(true); break;
        case "snapshots": brewfiles.load(); break;
        case "services":  services.load(true); break;
        case "discover":  if (search.query) search.run(search.query); break;
      }
      return;
    }

    // Esc: priority: settings (handles its own Esc) → palette → modal → detail.
    // Settings.svelte handles its own Esc listener with stopPropagation, so
    // we never reach this branch while Settings is open. Belt-and-suspenders
    // gate kept to make the priority intent obvious.
    if (e.key === "Escape") {
      if (ui.settingsOpen) return; // Settings handles its own Esc
      if (ui.paletteOpen) { ui.closePalette(); return; }
      if (ui.selectedPackage) { ui.closeDetail(); return; }
    }

    // "/": focus the in-view filter input (unless typing)
    if (e.key === "/" && !isTextInput(e.target)) {
      const input = document.querySelector<HTMLInputElement>('input[type="text"], input[type="search"], input:not([type])');
      if (input) {
        e.preventDefault();
        input.focus();
      }
    }
  }

  // Recompute the live max (60% of window) on window resize so a previously
  // valid width gets clamped back into range if the user shrinks the window.
  let windowWidth = $state(typeof window === "undefined" ? 1100 : window.innerWidth);
  let detailPaneMax = $derived(Math.max(DETAIL_PANE_MIN_WIDTH, Math.floor(windowWidth * 0.6)));

  onMount(() => {
    window.addEventListener("keydown", onKeydown);
    ui.loadDetailPaneWidthFromStorage();
    const onResize = () => {
      windowWidth = window.innerWidth;
      // Re-clamp current width against the new window dimensions.
      const clamped = clampDetailPaneWidth(ui.detailPaneWidth);
      if (clamped !== ui.detailPaneWidth) ui.setDetailPaneWidth(clamped);
    };
    window.addEventListener("resize", onResize);
    return () => {
      window.removeEventListener("keydown", onKeydown);
      window.removeEventListener("resize", onResize);
    };
  });
</script>

<div class="app">
  <div class="main">
    <Sidebar />
    <main class="content">
      {#key ui.section}
        <div class="section-pane">
          {#if ui.section === "dashboard"}
            <Dashboard />
          {:else if ui.section === "library"}
            <Library />
          {:else if ui.section === "discover"}
            <Discover />
          {:else if ui.section === "trending"}
            <Trending />
          {:else if ui.section === "snapshots"}
            <Snapshots />
          {:else if ui.section === "services"}
            <Services />
          {:else if ui.section === "activity"}
            <ActivityHistory />
          {/if}
        </div>
      {/key}
    </main>
    {#if ui.selectedPackage}
      <ResizeHandle
        width={ui.detailPaneWidth}
        min={DETAIL_PANE_MIN_WIDTH}
        max={detailPaneMax}
        defaultWidth={DETAIL_PANE_DEFAULT_WIDTH}
        direction="left"
        label="Resize package detail panel"
        onChange={(w) => (ui.detailPaneWidth = w)}
        onCommit={(w) => ui.setDetailPaneWidth(w)}
      />
      <PackageDetail />
    {/if}
  </div>
  <ActivityDrawer />
  <CommandPalette />
  <Settings />
  <DeviceFlowModal />
  <Toast />
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--color-surface);
  }
  .main {
    flex: 1;
    display: flex;
    min-height: 0;
    overflow: hidden;
  }
  .content {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background: var(--color-surface);
    overflow: hidden;
  }
  /* Quiet crossfade when switching sidebar sections.
     Tabs are peers, so we fade content rather than slide (designSystem §6). */
  .section-pane {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
    animation: section-in var(--motion-duration-base) var(--motion-ease-out);
  }
  @keyframes section-in {
    from { opacity: 0; }
    to   { opacity: 1; }
  }
  @media (prefers-reduced-motion: reduce) {
    .section-pane { animation: none; }
  }
</style>
