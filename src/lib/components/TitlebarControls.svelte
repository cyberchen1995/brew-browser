<script lang="ts">
  import { onMount } from "svelte";
  import Sun from "@lucide/svelte/icons/sun";
  import Moon from "@lucide/svelte/icons/moon";
  import Monitor from "@lucide/svelte/icons/monitor";
  import SettingsIcon from "@lucide/svelte/icons/settings";
  import Heart from "@lucide/svelte/icons/heart";
  import Check from "@lucide/svelte/icons/check";

  import { ui } from "$lib/stores/ui.svelte";
  import { safeOpenUrl } from "$lib/util/url";
  import { SPONSOR_URL } from "$lib/util/donate";
  import type { ThemePreference } from "$lib/types";

  /**
   * Title-bar right cluster: theme dropdown + Settings + Donate, grouped
   * as one rounded pill in the top-right of the window title bar. The
   * theme picker is a single button showing the current theme icon; click
   * opens a small popover with Light / Dark / System.
   */

  let themeOpen = $state(false);
  let themeBtn: HTMLButtonElement | undefined = $state();
  let themePopover: HTMLDivElement | undefined = $state();

  function activeIcon(t: ThemePreference) {
    return t === "light" ? Sun : t === "dark" ? Moon : Monitor;
  }
  function activeLabel(t: ThemePreference) {
    return t === "light" ? "Light" : t === "dark" ? "Dark" : "System";
  }

  let ActiveIcon = $derived(activeIcon(ui.theme));

  function pickTheme(t: ThemePreference) {
    ui.setTheme(t);
    themeOpen = false;
  }

  function onDocClick(e: MouseEvent) {
    if (!themeOpen) return;
    const t = e.target as Node | null;
    if (t && !themeBtn?.contains(t) && !themePopover?.contains(t)) {
      themeOpen = false;
    }
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape" && themeOpen) {
      themeOpen = false;
      themeBtn?.focus();
    }
  }

  function openSponsor() { void safeOpenUrl(SPONSOR_URL); }

  onMount(() => {
    document.addEventListener("click", onDocClick);
    window.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("click", onDocClick);
      window.removeEventListener("keydown", onKey);
    };
  });
</script>

<div class="cluster" data-tauri-drag-region="false" role="group" aria-label="App controls">
  <button
    bind:this={themeBtn}
    type="button"
    class="ctrl"
    class:open={themeOpen}
    onclick={() => (themeOpen = !themeOpen)}
    title={`Theme: ${activeLabel(ui.theme)}`}
    aria-label="Change theme"
    aria-haspopup="menu"
    aria-expanded={themeOpen}
  >
    <ActiveIcon size={14} />
  </button>
  <button
    type="button"
    class="ctrl"
    onclick={() => ui.openSettings()}
    title="Settings (⌘,)"
    aria-label="Open Settings"
  >
    <SettingsIcon size={14} />
  </button>
  <button
    type="button"
    class="ctrl donate"
    onclick={openSponsor}
    title="Donate via GitHub Sponsors"
    aria-label="Donate to brew-browser on GitHub Sponsors"
  >
    <Heart size={14} fill="currentColor" />
  </button>

  {#if themeOpen}
    <div
      bind:this={themePopover}
      class="popover"
      role="menu"
      aria-label="Theme"
    >
      <button
        type="button"
        class="popover-item"
        class:active={ui.theme === "light"}
        role="menuitemradio"
        aria-checked={ui.theme === "light"}
        onclick={() => pickTheme("light")}
      >
        <Sun size={14} />
        <span>Light</span>
        {#if ui.theme === "light"}<Check size={12} class="check" />{/if}
      </button>
      <button
        type="button"
        class="popover-item"
        class:active={ui.theme === "dark"}
        role="menuitemradio"
        aria-checked={ui.theme === "dark"}
        onclick={() => pickTheme("dark")}
      >
        <Moon size={14} />
        <span>Dark</span>
        {#if ui.theme === "dark"}<Check size={12} class="check" />{/if}
      </button>
      <button
        type="button"
        class="popover-item"
        class:active={ui.theme === "system"}
        role="menuitemradio"
        aria-checked={ui.theme === "system"}
        onclick={() => pickTheme("system")}
      >
        <Monitor size={14} />
        <span>System</span>
        {#if ui.theme === "system"}<Check size={12} class="check" />{/if}
      </button>
    </div>
  {/if}
</div>

<style>
  /* Pill-shaped group sitting on the sidebar-colored title bar. The
     background uses the panel-body gray (--color-surface) — same as
     the main content — so the cluster reads as a soft well rather
     than a hard black pill. Hair-line dividers between buttons. */
  .cluster {
    position: relative;
    display: inline-flex;
    align-items: center;
    background: var(--color-surface);
    border-radius: var(--radius-md);
    padding: 2px;
    gap: 0;
  }
  .ctrl {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 22px;
    background: transparent;
    border-radius: var(--radius-sm);
    color: var(--color-text-muted);
    cursor: pointer;
    transition: background-color var(--motion-duration-fast) var(--motion-ease-out),
                color var(--motion-duration-fast) var(--motion-ease-out);
  }
  .ctrl:hover,
  .ctrl.open {
    background: var(--color-surface-raised);
    color: var(--color-text-primary);
  }
  /* Hair-line divider between adjacent controls. */
  .ctrl + .ctrl { position: relative; }
  .ctrl + .ctrl::before {
    content: "";
    position: absolute;
    left: -1px;
    top: 4px;
    bottom: 4px;
    width: 1px;
    background: var(--color-border);
    opacity: 0.6;
  }
  /* Pink-filled heart for the Donate button. */
  .ctrl.donate { color: #ec4899; }
  .ctrl.donate:hover { color: #db2777; }

  /* Theme dropdown popover. */
  .popover {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    min-width: 140px;
    padding: 4px;
    background: var(--color-surface-raised);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    box-shadow: 0 8px 24px -4px color-mix(in oklch, black 30%, transparent);
    z-index: 40;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .popover-item {
    display: grid;
    grid-template-columns: 16px 1fr 14px;
    gap: var(--space-2);
    align-items: center;
    padding: 6px 8px;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--color-text-primary);
    text-align: left;
    font-size: var(--text-body-sm);
    cursor: pointer;
  }
  .popover-item:hover { background: var(--color-surface-sunken); }
  .popover-item :global(.check) { color: var(--color-accent); }
</style>
