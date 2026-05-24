<script lang="ts">
  /**
   * SettingsSectionBrew.svelte — Phase 12b
   *
   * - Analytics toggle: reads the user's Homebrew analytics posture via
   *   `brewGetAnalytics()` on mount, writes it back via `brewSetAnalytics`
   *   on toggle. Shows an in-flight state while the write is shelling out
   *   so the user can see something happened.
   * - Confirm-destructive toggle: writes through `ui.setConfirmDestructive`
   *   to localStorage so the preference survives restart.
   */

  import { onMount } from "svelte";

  import { brewGetAnalytics, brewSetAnalytics } from "$lib/api";
  import { ui } from "$lib/stores/ui.svelte";
  import { isBrewError, brewErrorMessage } from "$lib/types";
  import { toast } from "$lib/stores/toast.svelte";

  let analyticsEnabled = $state<boolean | null>(null);
  let analyticsLoading = $state(false);
  let analyticsError = $state<string | null>(null);

  async function loadAnalytics() {
    analyticsLoading = true;
    analyticsError = null;
    try {
      analyticsEnabled = await brewGetAnalytics();
    } catch (e) {
      analyticsError = isBrewError(e) ? brewErrorMessage(e) : String(e);
      // Surface a toast as well so the user notices even if they navigated
      // away from the Brew section before the probe resolved.
      toast.error("Couldn't read brew analytics state", analyticsError);
    } finally {
      analyticsLoading = false;
    }
  }

  async function toggleAnalytics() {
    if (analyticsLoading || analyticsEnabled === null) return;
    const next = !analyticsEnabled;
    analyticsLoading = true;
    analyticsError = null;
    try {
      await brewSetAnalytics(next);
      analyticsEnabled = next;
      toast.success(`Homebrew analytics ${next ? "enabled" : "disabled"}`);
    } catch (e) {
      analyticsError = isBrewError(e) ? brewErrorMessage(e) : String(e);
      toast.error("Couldn't change brew analytics", analyticsError);
      // Re-probe so the UI reflects whatever brew actually settled on.
      await loadAnalytics();
    } finally {
      analyticsLoading = false;
    }
  }

  onMount(() => {
    void loadAnalytics();
  });
</script>

<div class="section">
  <h2>Brew</h2>

  <section class="group">
    <h3>Analytics</h3>
    <p class="desc">
      Homebrew sends anonymous install analytics to formulae.brew.sh by
      default. This toggle flips Homebrew's setting (the same as running
      <code>brew analytics on</code> / <code>off</code> in your terminal).
    </p>

    <div class="row">
      <label class="toggle">
        <input
          type="checkbox"
          checked={analyticsEnabled === true}
          disabled={analyticsLoading || analyticsEnabled === null}
          onchange={toggleAnalytics}
        />
        <span>Send Homebrew install analytics</span>
      </label>
      {#if analyticsLoading}
        <span class="status">working…</span>
      {:else if analyticsEnabled === null && analyticsError}
        <span class="status status--err">unavailable</span>
      {/if}
    </div>

    {#if analyticsError && analyticsEnabled === null}
      <p class="err">{analyticsError}</p>
    {/if}
  </section>

  <section class="group">
    <h3>Confirmations</h3>
    <p class="desc">
      Destructive actions (Uninstall, Zap, Delete Brewfile) ask before
      proceeding. Power users can turn this off once they're sure.
    </p>
    <div class="row">
      <label class="toggle">
        <input
          type="checkbox"
          checked={ui.confirmDestructive}
          onchange={(e) => ui.setConfirmDestructive((e.currentTarget as HTMLInputElement).checked)}
        />
        <span>Confirm before uninstall / zap</span>
      </label>
    </div>
  </section>
</div>

<style>
  .section { display: flex; flex-direction: column; gap: var(--space-5); max-width: 560px; }
  h2 {
    font-size: var(--text-h1);
    font-weight: var(--fw-semibold);
    color: var(--color-text-primary);
    margin-bottom: var(--space-2);
  }
  .group { display: flex; flex-direction: column; gap: var(--space-2); }
  h3 {
    font-size: var(--text-h2);
    font-weight: var(--fw-semibold);
    color: var(--color-text-primary);
  }
  .desc {
    font-size: var(--text-body-sm);
    color: var(--color-text-secondary);
    line-height: var(--lh-normal);
  }
  .desc code {
    font-family: var(--font-mono);
    font-size: var(--text-mono);
    background: var(--color-surface-sunken);
    padding: 1px 4px;
    border-radius: var(--radius-sm);
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-2) 0;
  }
  .toggle {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    cursor: pointer;
    font-size: var(--text-body);
    color: var(--color-text-primary);
  }
  .toggle input[type="checkbox"] {
    width: 16px;
    height: 16px;
    accent-color: var(--color-brand);
    cursor: pointer;
  }
  .toggle input[type="checkbox"]:disabled { cursor: wait; }
  .status {
    font-size: var(--text-caption);
    color: var(--color-text-muted);
    font-style: italic;
  }
  .status--err { color: var(--color-danger); font-style: normal; }
  .err {
    font-size: var(--text-body-sm);
    color: var(--color-danger);
    background: var(--color-danger-subtle);
    border: 1px solid var(--color-danger);
    border-radius: var(--radius-sm);
    padding: var(--space-2) var(--space-3);
  }
</style>
