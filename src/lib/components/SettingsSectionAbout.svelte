<script lang="ts">
  /**
   * SettingsSectionAbout.svelte — Phase 12b
   *
   * App version (from `tauri::App::package_info`), brew version (from the
   * `env` store), MIT license, repo link via `safeOpenUrl`. Ends with the
   * zero-telemetry / zero-accounts affirmation paragraph.
   */

  import { onMount } from "svelte";
  import ExternalLink from "@lucide/svelte/icons/external-link";

  import { appVersion } from "$lib/api";
  import { env } from "$lib/stores/env.svelte";
  import { safeOpenUrl } from "$lib/util/url";

  const REPO_URL = "https://github.com/msitarzewski/brew-browser";

  let version = $state<string | null>(null);
  let versionError = $state<string | null>(null);

  onMount(() => {
    void appVersion()
      .then((v) => (version = v))
      .catch((e) => (versionError = String(e)));
  });
</script>

<div class="section">
  <h2>About</h2>

  <dl class="meta">
    <div class="row">
      <dt>App version</dt>
      <dd class="mono">
        {#if version}{version}{:else if versionError}—{:else}…{/if}
      </dd>
    </div>
    <div class="row">
      <dt>Homebrew</dt>
      <dd class="mono">{env.report?.version ?? "—"}</dd>
    </div>
    <div class="row">
      <dt>License</dt>
      <dd>MIT</dd>
    </div>
    <div class="row">
      <dt>Repository</dt>
      <dd>
        <button class="link" type="button" onclick={() => void safeOpenUrl(REPO_URL)}>
          <code>github.com/msitarzewski/brew-browser</code>
          <ExternalLink size={12} />
        </button>
      </dd>
    </div>
  </dl>

  <div class="affirm">
    <h3>Zero telemetry. Zero accounts.</h3>
    <p>
      brew-browser does not collect telemetry. It does not phone home. It does
      not have user accounts. Every outbound network request is documented in
      Settings → Network and the README, and only fires when you take an
      action that requires it.
    </p>
  </div>
</div>

<style>
  .section { display: flex; flex-direction: column; gap: var(--space-5); max-width: 560px; }
  h2 {
    font-size: var(--text-h1);
    font-weight: var(--fw-semibold);
    color: var(--color-text-primary);
    margin-bottom: var(--space-2);
  }
  .meta {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding: var(--space-3) var(--space-4);
    background: var(--color-surface-sunken);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
  }
  .row {
    display: grid;
    grid-template-columns: 120px 1fr;
    gap: var(--space-3);
    padding: 4px 0;
    align-items: baseline;
  }
  dt {
    font-size: var(--text-body-sm);
    color: var(--color-text-muted);
    font-weight: var(--fw-medium);
  }
  dd {
    font-size: var(--text-body);
    color: var(--color-text-primary);
  }
  .mono { font-family: var(--font-mono); font-size: var(--text-mono); }
  .link {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--color-text-link);
    font-size: var(--text-body-sm);
    cursor: pointer;
    background: transparent;
    padding: 0;
  }
  .link:hover { text-decoration: underline; }
  .link code {
    font-family: var(--font-mono);
    font-size: var(--text-mono);
    color: inherit;
  }
  .affirm {
    padding: var(--space-4);
    background: var(--color-info-subtle);
    border: 1px solid var(--color-info);
    border-radius: var(--radius-md);
  }
  .affirm h3 {
    font-size: var(--text-h2);
    font-weight: var(--fw-semibold);
    color: var(--color-text-primary);
    margin-bottom: var(--space-2);
  }
  .affirm p {
    font-size: var(--text-body-sm);
    color: var(--color-text-secondary);
    line-height: var(--lh-normal);
  }
</style>
