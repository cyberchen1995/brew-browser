<script lang="ts">
  /**
   * SettingsSectionVulnerabilities.svelte — v0.5.0
   *
   * Sibling of SettingsSectionTrendingHistory / SettingsSectionUpdates:
   * embedded near the bottom of SettingsSectionNetwork. The opt-in
   * toggle for the `brew vulns`-backed vulnerability scan path.
   *
   * Why a separate section? The scan path crosses TWO distinct trust
   * boundaries vs the always-on Homebrew analytics:
   *   - OSV.dev (Google) — the underlying advisory feed `brew vulns`
   *     consults.
   *   - api.github.com (when GitHub auth is enabled) — secondary
   *     enrichment for GHSA-id details.
   * The disclosure copy makes both explicit so users opt in knowingly.
   *
   * Five visible states:
   *   1. Off (default) — short paragraph + toggle. No buttons.
   *   2. On, helper not installed (detected via `vulns_not_installed`
   *      error) — install affordance: "Install brew-vulns" button.
   *   3. On, installed, no scan yet — "Scan now" button +
   *      "Last scan: never".
   *   4. On, installed, after scan — status line with vuln counts +
   *      "Scan now" (refresh). Positive state when zero findings.
   *   5. On, lastScanError set — danger callout under the buttons.
   *
   * Detection of "is brew-vulns installed?" is implicit — the store
   * doesn't track it directly. We use `lastScanError` containing
   * "vulns_not_installed" as the signal; otherwise we assume installed
   * once any scan has completed.
   *
   * Offline Mode (paranoid_mode) hard-locks the feature even when the
   * toggle is on — same gating contract as TrendingHistory.
   */

  import ShieldAlert from "@lucide/svelte/icons/shield-alert";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import Loader from "@lucide/svelte/icons/loader-2";
  import Download from "@lucide/svelte/icons/download";
  import CheckCircle from "@lucide/svelte/icons/check-circle-2";
  import TriangleAlert from "@lucide/svelte/icons/triangle-alert";

  import { settings } from "$lib/stores/settings.svelte";
  import { vulnerabilities } from "$lib/stores/vulnerabilities.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { reportableToastError } from "$lib/util/reportIssue";

  /** Offline Mode locks the feature off regardless of toggle state. */
  let offline = $derived(settings.effective.paranoidMode);

  /** Effective toggle state — used for visual + disclosure logic. */
  let on = $derived(settings.effective.vulnerabilityScanningEnabled);

  /** Whether the last scan failed because the `brew vulns` subcommand
      isn't installed. Drives the install-affordance swap. The store
      stamps this exact phrase into `lastScanError` for us. */
  let helperMissing = $derived(
    vulnerabilities.lastScanError !== null &&
    vulnerabilities.lastScanError.includes("not installed"),
  );

  /** True once we've completed at least one scan without a
      `vulns_not_installed` error — implies brew-vulns is present. */
  let helperConfirmedInstalled = $derived(
    vulnerabilities.lastScannedAt !== null && !helperMissing,
  );

  /** Local flag for the install button so we can show a spinner while
      the brew install runs. The store doesn't track helper-install
      progress — that's a UI-local concern. */
  let installing = $state(false);

  let counts = $derived(vulnerabilities.severityCounts);
  let lastScannedAt = $derived(vulnerabilities.lastScannedAt);
  let scanSource = $derived(vulnerabilities.source);

  /** Relative-time formatter for "Last scan: 3 minutes ago". Avoids
      pulling in dayjs/date-fns just for this — Intl handles it. */
  const RELATIVE_TIME = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });

  function relativeTime(d: Date | null): string {
    if (!d) return "never";
    const deltaSec = Math.round((d.getTime() - Date.now()) / 1000);
    const abs = Math.abs(deltaSec);
    if (abs < 60) return RELATIVE_TIME.format(deltaSec, "second");
    if (abs < 3600) return RELATIVE_TIME.format(Math.round(deltaSec / 60), "minute");
    if (abs < 86400) return RELATIVE_TIME.format(Math.round(deltaSec / 3600), "hour");
    return RELATIVE_TIME.format(Math.round(deltaSec / 86400), "day");
  }

  let lastScanLabel = $derived(relativeTime(lastScannedAt));

  async function onToggle(e: Event) {
    const next = (e.currentTarget as HTMLInputElement).checked;
    await settings.save({ vulnerabilityScanningEnabled: next });
    if (next && !vulnerabilities.lastScannedAt) {
      // First enable → kick off an auto-scan so the UI is populated
      // by the time the user looks at the Dashboard or Library.
      await vulnerabilities.scanAll(false);
    }
    if (!next) {
      vulnerabilities.clear();
    }
  }

  async function onScanNow() {
    await vulnerabilities.scanAll(true);
  }

  async function onInstallHelper() {
    if (installing) return;
    installing = true;
    try {
      await vulnerabilities.installHelper();
      toast.success(
        "brew-vulns installed",
        "Running an initial scan now…",
      );
      // Helper is now present — kick a forced scan so the empty state
      // flips to real data immediately.
      await vulnerabilities.scanAll(true);
    } catch (e) {
      reportableToastError("Couldn't install brew-vulns", e);
    } finally {
      installing = false;
    }
  }
</script>

<div class="section">
  <h2>
    <ShieldAlert size={18} aria-hidden="true" />
    Vulnerability Scanning
  </h2>

  <div class="field">
    <label
      class="toggle"
      title={offline ? "Disabled by Offline Mode" : undefined}
    >
      <input
        type="checkbox"
        checked={on}
        onchange={onToggle}
        disabled={offline || settings.loading || settings.corruptOnDisk}
        aria-describedby="vuln-scan-hint"
      />
      <span class="toggle-track" aria-hidden="true"></span>
      <span class="toggle-label">Scan installed packages for known vulnerabilities</span>
    </label>

    <p class="hint" id="vuln-scan-hint">
      Opt-in, off by default. When on, brew-browser shells out to the
      official <code>brew vulns</code> subcommand (Homebrew project), which
      queries <code>OSV.dev</code> (operated by Google) for known
      vulnerabilities affecting your installed formulae. If you're also
      signed in to GitHub, individual GHSA-IDs are enriched with details
      from <code>api.github.com</code>. Findings are cached locally; no
      package list leaves your machine except the queries
      <code>brew vulns</code> itself makes to OSV.
    </p>

    {#if offline}
      <p class="hint hint-warn">
        Offline Mode is on — vulnerability scanning is suppressed even if
        this toggle is on. Disable Offline Mode above to enable scanning.
      </p>
    {/if}
  </div>

  <!-- Action area, only relevant when the feature is on and Offline
       Mode isn't blocking. Renders one of three sub-states: install
       affordance, ready-to-scan, or post-scan status. -->
  {#if on && !offline}
    {#if helperMissing}
      <!-- State 2: helper not installed. -->
      <div class="callout install" role="region" aria-label="Install brew-vulns">
        <div class="callout-head">
          <TriangleAlert size={16} />
          <strong>The brew-vulns subcommand isn't installed.</strong>
        </div>
        <p class="callout-body">
          Vulnerability scanning needs the official
          <code>brew vulns</code> subcommand. Install it now? This runs
          <code>brew install homebrew/brew-vulns/brew-vulns</code>.
        </p>
        <div class="row">
          <button
            type="button"
            class="btn-primary"
            onclick={onInstallHelper}
            disabled={installing}
          >
            {#if installing}
              <span class="spin"><Loader size={14} /></span>
              Installing…
            {:else}
              <Download size={14} />
              Install brew-vulns
            {/if}
          </button>
        </div>
      </div>
    {:else}
      <!-- States 3 + 4: idle (never scanned) or post-scan status. -->
      <div class="field">
        <div class="row">
          <button
            type="button"
            class="btn-secondary"
            onclick={onScanNow}
            disabled={vulnerabilities.loading}
            title="Run brew vulns against every installed formula"
          >
            {#if vulnerabilities.loading}
              <span class="spin"><Loader size={14} /></span>
              Scanning…
            {:else}
              <RefreshCw size={14} />
              Scan now
            {/if}
          </button>
          <span class="meta">Last scan: {lastScanLabel}</span>
        </div>

        {#if helperConfirmedInstalled}
          {#if counts.vulnerablePackages === 0 && counts.total === 0}
            <!-- Clean result: positive framing, this is the GOOD case. -->
            <div class="callout clean" role="status">
              <CheckCircle size={16} />
              <span>
                No known vulnerabilities across your installed packages.
                {#if scanSource}
                  <span class="meta-inline">(source: {scanSource})</span>
                {/if}
              </span>
            </div>
          {:else}
            <!-- Found some. Break out by severity tier. -->
            <p class="status-line">
              <strong>{counts.vulnerablePackages}</strong>
              package{counts.vulnerablePackages === 1 ? "" : "s"} with known
              vulnerabilities ·
              <span class="sev sev-danger">{counts.critical} critical</span> ·
              <span class="sev sev-danger">{counts.high} high</span> ·
              <span class="sev sev-warning">{counts.medium} medium</span> ·
              <span class="sev sev-info">{counts.low} low</span>
              {#if counts.unknown > 0}
                · <span class="sev sev-neutral">{counts.unknown} unknown</span>
              {/if}
              {#if scanSource}
                <span class="meta-inline">· source: {scanSource}</span>
              {/if}
            </p>
          {/if}
        {/if}
      </div>
    {/if}

    <!-- Surface the last error (other than `vulns_not_installed`,
         which has its own affordance above). -->
    {#if vulnerabilities.lastScanError && !helperMissing}
      <div class="callout error" role="alert">
        <TriangleAlert size={16} />
        <span>{vulnerabilities.lastScanError}</span>
      </div>
    {/if}
  {/if}
</div>

<style>
  /* Mirrors SettingsSectionTrendingHistory: nested subsection with a
     divider on top, no second-tier H1. */
  .section {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    max-width: 580px;
    margin-top: var(--space-3);
    padding-top: var(--space-5);
    border-top: 1px solid var(--color-border);
  }
  h2 {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    font-size: var(--text-h2);
    font-weight: var(--fw-semibold);
    color: var(--color-text-primary);
    margin: 0 0 var(--space-2) 0;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .hint {
    font-size: var(--text-body-sm);
    color: var(--color-text-muted);
    line-height: var(--lh-snug);
  }
  .hint code {
    font-family: var(--font-mono);
    font-size: var(--text-mono);
    padding: 1px 4px;
    background: var(--color-surface-sunken);
    border-radius: var(--radius-sm);
    color: var(--color-text-secondary);
    word-break: break-all;
  }
  .hint-warn {
    color: var(--color-warning-strong, #b45309);
  }
  .row {
    display: inline-flex;
    align-items: center;
    gap: var(--space-3);
    flex-wrap: wrap;
  }
  .meta {
    font-size: var(--text-body-sm);
    color: var(--color-text-muted);
  }
  .meta-inline {
    color: var(--color-text-muted);
    font-size: var(--text-body-sm);
    margin-left: 4px;
  }
  .status-line {
    font-size: var(--text-body-sm);
    color: var(--color-text-secondary);
    line-height: var(--lh-normal);
  }
  .status-line strong {
    color: var(--color-text-primary);
    font-weight: var(--fw-semibold);
  }
  .sev {
    display: inline-block;
    padding: 1px 6px;
    border-radius: var(--radius-sm);
    font-weight: var(--fw-medium);
    font-variant-numeric: tabular-nums;
  }
  .sev-danger  { background: var(--color-danger-subtle);  color: var(--color-danger-on-subtle); }
  .sev-warning { background: var(--color-warning-subtle); color: var(--color-warning-on-subtle); }
  .sev-info    { background: var(--color-info-subtle);    color: var(--color-info-on-subtle); }
  .sev-neutral { background: var(--color-surface-sunken); color: var(--color-text-secondary); }

  /* ---------- Toggle (matches Network/TrendingHistory) ---------- */
  .toggle {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    cursor: pointer;
    user-select: none;
  }
  .toggle input { position: absolute; opacity: 0; pointer-events: none; }
  .toggle-track {
    width: 36px;
    height: 20px;
    background: var(--color-surface-sunken);
    border: 1px solid var(--color-border);
    border-radius: 999px;
    position: relative;
    transition: background-color var(--motion-duration-fast) var(--motion-ease-out);
  }
  .toggle-track::after {
    content: "";
    position: absolute;
    top: 1px;
    left: 1px;
    width: 16px;
    height: 16px;
    background: var(--color-surface-raised);
    border-radius: 50%;
    box-shadow: var(--shadow-xs);
    transition: transform var(--motion-duration-fast) var(--motion-ease-out);
  }
  .toggle input:checked + .toggle-track {
    background: var(--color-accent, #b8542a);
    border-color: var(--color-accent, #b8542a);
  }
  .toggle input:checked + .toggle-track::after {
    transform: translateX(16px);
    background: white;
  }
  .toggle input:disabled + .toggle-track {
    opacity: 0.6;
    cursor: not-allowed;
  }
  .toggle-label {
    font-size: var(--text-body);
    font-weight: var(--fw-medium);
    color: var(--color-text-primary);
  }

  /* ---------- Buttons (match Updates section pattern) ---------- */
  .btn-primary,
  .btn-secondary {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border-radius: var(--radius-md);
    font-size: var(--text-body-sm);
    font-weight: var(--fw-medium);
    cursor: pointer;
    width: max-content;
  }
  .btn-primary {
    background: var(--color-accent, #b8542a);
    color: white;
  }
  .btn-primary:hover:not(:disabled) { filter: brightness(1.05); }
  .btn-primary:disabled { opacity: 0.6; cursor: not-allowed; }
  .btn-secondary {
    background: var(--color-surface-raised);
    color: var(--color-text-primary);
    border: 1px solid var(--color-border);
  }
  .btn-secondary:hover:not(:disabled) { background: var(--color-surface); }
  .btn-secondary:disabled { opacity: 0.6; cursor: not-allowed; }

  /* ---------- Callouts (install / clean / error) ---------- */
  .callout {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-3);
    border-radius: var(--radius-md);
    border: 1px solid var(--color-border);
  }
  .callout-head {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--color-text-primary);
    font-size: var(--text-body);
  }
  .callout-body {
    font-size: var(--text-body-sm);
    color: var(--color-text-secondary);
    line-height: var(--lh-snug);
  }
  .callout-body code {
    font-family: var(--font-mono);
    font-size: var(--text-mono);
    padding: 1px 4px;
    background: var(--color-surface-sunken);
    border-radius: var(--radius-sm);
  }
  .install {
    background: var(--color-warning-subtle);
    border-color: var(--color-warning-subtle);
  }
  .install .callout-head { color: var(--color-warning-on-subtle); }
  .clean {
    flex-direction: row;
    align-items: center;
    background: var(--color-success-subtle);
    border-color: var(--color-success-subtle);
    color: var(--color-success-on-subtle);
    font-size: var(--text-body-sm);
  }
  .error {
    flex-direction: row;
    align-items: center;
    background: var(--color-danger-subtle);
    border-color: var(--color-danger-subtle);
    color: var(--color-danger-on-subtle);
    font-size: var(--text-body-sm);
  }

  /* Spinner used inline in buttons. */
  .spin {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    animation: spin 1s linear infinite;
  }
  @keyframes spin {
    from { transform: rotate(0deg); }
    to   { transform: rotate(360deg); }
  }
  @media (prefers-reduced-motion: reduce) {
    .spin { animation: none; }
  }
</style>
