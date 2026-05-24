<script lang="ts">
  import { onMount, untrack } from "svelte";
  import X from "@lucide/svelte/icons/x";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import Download from "@lucide/svelte/icons/download";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import RefreshCcw from "@lucide/svelte/icons/refresh-ccw";
  import ArrowUpCircle from "@lucide/svelte/icons/arrow-up-circle";

  import Pill from "./Pill.svelte";
  import Button from "./Button.svelte";
  import DestructiveConfirm from "./DestructiveConfirm.svelte";
  import LoadingState from "./LoadingState.svelte";
  import Play from "@lucide/svelte/icons/play";
  import Square from "@lucide/svelte/icons/square";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import Star from "@lucide/svelte/icons/star";
  import GitFork from "@lucide/svelte/icons/git-fork";
  import Tag from "@lucide/svelte/icons/tag";
  import Archive from "@lucide/svelte/icons/archive";
  import Loader from "@lucide/svelte/icons/loader-2";
  import AlertCircle from "@lucide/svelte/icons/alert-circle";

  import { ui } from "$lib/stores/ui.svelte";
  import { packages } from "$lib/stores/packages.svelte";
  import { activity } from "$lib/stores/activity.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { categories } from "$lib/stores/categories.svelte";
  import { discover } from "$lib/stores/discover.svelte";
  import { services } from "$lib/stores/services.svelte";
  import { settings } from "$lib/stores/settings.svelte";
  import { github, type RepoStatsOutcome } from "$lib/stores/github.svelte";
  import { brewInfo, brewInstall, brewUninstall, brewUpgrade } from "$lib/api";
  import { safeOpenUrl } from "$lib/util/url";
  import { resolveCategoryIcon } from "$lib/util/categoryIcon";
  import { isBrewError, normalizeServiceStatus, type IconSource, type PackageDetail } from "$lib/types";

  // Categories file is small; ensure it's loaded so the pills can render. Idempotent.
  categories.ensureLoaded();

  // Small transparency label for the meta row — keeps "where did this come from?"
  // visible without painting a whole section. Skips into a tooltip for the
  // homepage URL so the line itself stays one token wide.
  function iconSourceLabel(src: IconSource): string {
    switch (src.kind) {
      case "installedApp": return "installed app";
      case "homepage":     return "homepage";
      case "none":         return "none";
    }
  }
  function iconSourceTitle(src: IconSource): string | undefined {
    return src.kind === "homepage" ? `Favicon from ${src.homepage}` : undefined;
  }

  let detail = $state<PackageDetail | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);

  let depsOpen = $state(false);
  let dependentsOpen = $state(false);
  let confirmUninstall = $state(false);

  // Focus management for the slide-over (A11Y-2 / WCAG 2.4.3).
  // When the panel opens (null → truthy), capture the previously-focused element
  // and move focus to the panel heading. When it closes (truthy → null), restore.
  let headingEl: HTMLHeadingElement | undefined = $state();
  let openerEl: HTMLElement | null = null;
  let lastOpen = false;

  // Reload when selected package changes (also handles focus open/close transitions)
  $effect(() => {
    const sel = ui.selectedPackage;
    const isOpen = !!sel;

    if (isOpen && !lastOpen) {
      // null → truthy: opening
      const active = document.activeElement;
      openerEl = active instanceof HTMLElement ? active : null;
      queueMicrotask(() => headingEl?.focus());
    } else if (!isOpen && lastOpen) {
      // truthy → null: closing — restore focus to the opener
      const toFocus = openerEl;
      openerEl = null;
      queueMicrotask(() => toFocus?.focus?.());
    }
    lastOpen = isOpen;

    if (!sel) {
      untrack(() => {
        detail = null;
        error = null;
      });
      return;
    }
    untrack(() => loadDetail(sel.name, sel.kind));
  });

  async function loadDetail(name: string, kind: "formula" | "cask") {
    loading = true;
    error = null;
    detail = null;
    try {
      detail = await brewInfo(name, kind);
    } catch (e) {
      error = isBrewError(e) ? e.code : `Backend not available: ${String(e)}`;
    } finally {
      loading = false;
    }
  }

  async function doInstall() {
    if (!ui.selectedPackage) return;
    const { name, kind } = ui.selectedPackage;
    const tmpId = crypto.randomUUID();
    activity.startJob(`Installing ${name}`, tmpId, `brew install ${name}`);
    ui.openDrawer();
    try {
      const result = await brewInstall(name, kind, (evt) => {
        // first event will carry the real jobId; rewrite if needed
        if (evt.kind === "started" && evt.jobId !== tmpId) {
          const j = activity.jobs.find((j) => j.jobId === tmpId);
          if (j) j.jobId = evt.jobId;
        }
        activity.handleEvent(evt);
      });
      if (result.success) {
        toast.success(`Installed ${name}`);
        packages.load(true);
        if (ui.selectedPackage) loadDetail(ui.selectedPackage.name, ui.selectedPackage.kind);
      } else {
        toast.error(`Install failed: ${name}`);
      }
    } catch (e) {
      toast.error("Install failed", isBrewError(e) ? e.code : String(e));
    }
  }

  async function doUninstall() {
    if (!ui.selectedPackage) return;
    confirmUninstall = false;
    const { name, kind } = ui.selectedPackage;
    const tmpId = crypto.randomUUID();
    activity.startJob(`Uninstalling ${name}`, tmpId, `brew uninstall ${name}`);
    ui.openDrawer();
    try {
      const result = await brewUninstall(name, kind, false, (evt) => {
        if (evt.kind === "started" && evt.jobId !== tmpId) {
          const j = activity.jobs.find((j) => j.jobId === tmpId);
          if (j) j.jobId = evt.jobId;
        }
        activity.handleEvent(evt);
      });
      if (result.success) {
        toast.success(`Uninstalled ${name}`);
        packages.load(true);
        ui.closeDetail();
      } else {
        toast.error(`Uninstall failed: ${name}`);
      }
    } catch (e) {
      toast.error("Uninstall failed", isBrewError(e) ? e.code : String(e));
    }
  }

  async function doUpgrade() {
    if (!ui.selectedPackage) return;
    const { name } = ui.selectedPackage;
    const tmpId = crypto.randomUUID();
    activity.startJob(`Upgrading ${name}`, tmpId, `brew upgrade ${name}`);
    ui.openDrawer();
    try {
      const result = await brewUpgrade(name, (evt) => {
        if (evt.kind === "started" && evt.jobId !== tmpId) {
          const j = activity.jobs.find((j) => j.jobId === tmpId);
          if (j) j.jobId = evt.jobId;
        }
        activity.handleEvent(evt);
      });
      if (result.success) {
        toast.success(`Upgraded ${name}`);
        packages.load(true);
        if (ui.selectedPackage) loadDetail(ui.selectedPackage.name, ui.selectedPackage.kind);
      } else {
        toast.error(`Upgrade failed: ${name}`);
      }
    } catch (e) {
      toast.error("Upgrade failed", isBrewError(e) ? e.code : String(e));
    }
  }

  async function openHomepage(url: string) {
    // Scheme allowlist (http/https only) lives in safeOpenUrl — cask `homepage`
    // is attacker-influenced metadata; never hand a raw URL to the opener.
    // Security audit §H1 (memory-bank/security.md).
    await safeOpenUrl(url);
  }

  function close() { ui.closeDetail(); }

  // Helpful derived — explicit type to avoid `never` narrowing from union state.
  let pkg = $derived<PackageDetail["package"] | undefined>(detail?.package);
  let isInstalled = $derived(!!pkg?.installedVersion);
  let isOutdated = $derived(!!pkg?.outdated);

  /** Categories assigned to this package (from `categories.json`). */
  let pkgCategories = $derived.by<string[]>(() => {
    if (!pkg) return [];
    return categories.categoriesOf(pkg.name, pkg.kind);
  });

  /**
   * Jump to Discover with a category chip pre-selected. Closes the detail panel
   * so the user lands on the filtered view, not an obscured one.
   */
  function jumpToCategory(slug: string) {
    discover.selectOnly(slug);
    ui.closeDetail();
    ui.setSection("discover");
  }

  /** Brew service entry for this package, if it has one. Formulae only. */
  let svc = $derived.by(() =>
    pkg && pkg.kind === "formula" ? services.byName(pkg.name) : undefined,
  );
  let svcStatus = $derived(svc ? normalizeServiceStatus(svc.status) : null);
  let svcPending = $derived(pkg ? services.isPending(pkg.name) : false);

  // ────────────────────────────────────────────────────────────────
  // Phase 12c — GitHub stats card.
  //
  // We gate three things before triggering a fetch:
  //   1. `settings.effective.githubEnabled` — user opt-in toggle.
  //   2. `!settings.effective.paranoidMode` — master kill switch.
  //   3. `pkg.homepage` looks like a github.com URL (cheap pre-check
  //      so we don't even paint the card for non-GitHub homepages).
  //
  // The backend re-validates the URL strictly via `parse_github_url`,
  // so the frontend check is just a UI optimisation; if it false-
  // positives the backend will still return `null` (collapses to "miss").
  //
  // The store handles the actual fetch + caching; we just read the
  // outcome out of `github.repoStatsCache` and re-render.

  /** Cheap regex check: looks like https?://github.com/<owner>/<repo>. */
  function looksLikeGithubHomepage(url: string | null | undefined): boolean {
    if (!url) return false;
    return /^https?:\/\/github\.com\/[^/]+\/[^/?#]+/i.test(url.trim());
  }

  let githubStatsEligible = $derived(
    !!pkg?.homepage &&
      looksLikeGithubHomepage(pkg?.homepage) &&
      settings.effective.githubEnabled &&
      !settings.effective.paranoidMode,
  );

  /** The outcome from the GitHub store for the current homepage. */
  let githubOutcome = $derived<RepoStatsOutcome | null>(
    githubStatsEligible && pkg?.homepage
      ? (github.repoStatsCache.get(pkg.homepage) ?? { kind: "loading" })
      : null,
  );

  // Trigger the fetch on first paint for a given homepage. The store
  // memoises in its cache so repeat opens of the same package don't
  // re-invoke.
  $effect(() => {
    if (githubStatsEligible && pkg?.homepage) {
      void github.getRepoStats(pkg.homepage);
    }
  });

  /** Format a star/fork count in compact form (1.2k, 12.3k, 1.2M). */
  function fmtCount(n: number): string {
    if (n < 1000) return `${n}`;
    if (n < 1_000_000) return `${(n / 1000).toFixed(n < 10_000 ? 1 : 0).replace(/\.0$/, "")}k`;
    return `${(n / 1_000_000).toFixed(1).replace(/\.0$/, "")}M`;
  }

  /** "3 weeks ago" style for ISO timestamps. */
  function fmtRelative(iso: string | null): string {
    if (!iso) return "—";
    const t = Date.parse(iso);
    if (Number.isNaN(t)) return "—";
    const diffMs = Date.now() - t;
    const days = Math.max(0, Math.floor(diffMs / (24 * 60 * 60 * 1000)));
    if (days === 0) return "today";
    if (days === 1) return "yesterday";
    if (days < 7) return `${days} days ago`;
    if (days < 30) return `${Math.floor(days / 7)} weeks ago`;
    if (days < 365) return `${Math.floor(days / 30)} months ago`;
    const years = Math.floor(days / 365);
    return years === 1 ? "1 year ago" : `${years} years ago`;
  }

  /** Format the rate-limit reset time for the inline error variant. */
  function fmtResetTime(resetAt: number): string {
    if (!resetAt) return "soon";
    return new Date(resetAt * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }

  async function svcAct(action: "start" | "stop" | "restart") {
    if (!pkg) return;
    try {
      await services.act(pkg.name, action);
      toast.success(`${action.charAt(0).toUpperCase() + action.slice(1)}ed ${pkg.name}`);
    } catch (e) {
      toast.error(`Failed to ${action} ${pkg.name}`, isBrewError(e) ? e.code : String(e));
    }
  }
</script>

{#if ui.selectedPackage}
  <aside
    class="detail"
    aria-label="Package detail"
    style="--detail-pane-width: {ui.detailPaneWidth}px"
  >
    <header>
      <div class="head-left">
        <h1 bind:this={headingEl} tabindex="-1">{ui.selectedPackage.name}</h1>
        <Pill tone={ui.selectedPackage.kind === "formula" ? "formula" : "cask"}>{ui.selectedPackage.kind}</Pill>
      </div>
      <button class="close" aria-label="Close detail panel" onclick={close} title="Close (Esc)">
        <X size={16} />
      </button>
    </header>

    <div class="body">
      {#if loading}
        <LoadingState rows={5} label="Loading package detail…" />
      {:else if error}
        <div class="error">
          <p>Couldn't load detail: {error}</p>
          <Button variant="secondary" onclick={() => ui.selectedPackage && loadDetail(ui.selectedPackage.name, ui.selectedPackage.kind)}>Retry</Button>
        </div>
      {:else if detail && pkg}
        <dl class="meta">
          <div>
            <dt>Installed</dt>
            <dd>{pkg.installedVersion ?? "Not installed"}</dd>
          </div>
          <div>
            <dt>Latest</dt>
            <dd>
              {pkg.stableVersion ?? "—"}
              {#if isOutdated}
                <span class="warn">Upgrade available</span>
              {/if}
            </dd>
          </div>
          {#if pkg.license}
            <div><dt>License</dt><dd>{pkg.license}</dd></div>
          {/if}
          {#if pkg.tap}
            <div><dt>Tap</dt><dd>{pkg.tap}</dd></div>
          {/if}
          <div>
            <dt>Icon source</dt>
            <dd class="icon-source" title={iconSourceTitle(pkg.iconSource)}>{iconSourceLabel(pkg.iconSource)}</dd>
          </div>
          {#if pkgCategories.length > 0}
            <div>
              <dt>Categories</dt>
              <dd class="cat-pills">
                {#each pkgCategories as slug (slug)}
                  {@const Icon = resolveCategoryIcon(
                    categories.data?.categories[slug]?.icon ?? "HelpCircle",
                  )}
                  <button
                    type="button"
                    class="cat-pill"
                    onclick={() => jumpToCategory(slug)}
                    title={`Browse all packages in ${categories.labelOf(slug)}`}
                  >
                    <Icon size={12} />
                    <span>{categories.labelOf(slug)}</span>
                  </button>
                {/each}
              </dd>
            </div>
          {/if}
        </dl>

        {#if pkg.description}
          <p class="desc">{pkg.description}</p>
        {/if}

        {#if pkg.homepage}
          <button class="homepage" onclick={() => openHomepage(pkg!.homepage!)} title={pkg.homepage}>
            <span class="truncate">{pkg.homepage}</span>
            <ExternalLink size={12} />
          </button>
        {/if}

        {#if githubOutcome}
          <section class="gh-card" aria-label="GitHub repository statistics">
            {#if githubOutcome.kind === "loading"}
              <div class="gh-loading">
                <Loader size={14} class="spin-slow" />
                <span>Loading GitHub stats…</span>
              </div>
            {:else if githubOutcome.kind === "loaded"}
              {@const s = githubOutcome.stats}
              <div class="gh-stats">
                <span class="gh-stat" title="Stargazers">
                  <Star size={14} /> {fmtCount(s.stars)}
                </span>
                <span class="gh-sep">·</span>
                <span class="gh-stat" title="Forks">
                  <GitFork size={14} /> {fmtCount(s.forks)}
                </span>
                {#if s.lastReleaseTag}
                  <span class="gh-sep">·</span>
                  <span class="gh-stat" title={s.lastReleaseDate ?? ""}>
                    <Tag size={14} /> {s.lastReleaseTag}
                    {#if s.lastReleaseDate}
                      <span class="gh-muted">({fmtRelative(s.lastReleaseDate)})</span>
                    {/if}
                  </span>
                {/if}
              </div>
              {#if s.archived}
                <div class="gh-archived" role="status">
                  <Archive size={14} />
                  <span>
                    Archived{s.archivedAt ? ` ${fmtRelative(s.archivedAt)}` : ""} — likely unmaintained.
                  </span>
                </div>
              {/if}
              {#if s.licenseSpdx && pkg.license && s.licenseSpdx !== pkg.license}
                <div
                  class="gh-license-mismatch"
                  title={`brew reports: ${pkg.license} · GitHub reports: ${s.licenseSpdx}`}
                >
                  <AlertCircle size={12} />
                  License mismatch — brew: <code>{pkg.license}</code>, GitHub: <code>{s.licenseSpdx}</code>
                </div>
              {/if}
            {:else if githubOutcome.kind === "rateLimited"}
              <div class="gh-error">
                <AlertCircle size={14} />
                <span>
                  GitHub stats temporarily unavailable (rate limit resets at
                  {fmtResetTime(githubOutcome.resetAt)}).
                  Sign in via Settings → GitHub to remove the limit.
                </span>
              </div>
            {:else if githubOutcome.kind === "blocked"}
              <div class="gh-error">
                <AlertCircle size={14} />
                <span>Blocked by Paranoid Mode. Disable in Settings → Network.</span>
              </div>
            {:else if githubOutcome.kind === "error"}
              <div class="gh-error">
                <AlertCircle size={14} />
                <span>Couldn't load GitHub stats: {githubOutcome.message}</span>
              </div>
            {/if}
            <!-- kind === "miss" renders nothing -->
          </section>
        {/if}

        {#if svc}
          <section class="service-card" class:pending={svcPending}>
            <div class="svc-head">
              <h3>Service</h3>
              <Pill tone={svcStatus === "started" ? "success" : svcStatus === "error" ? "danger" : svcStatus === "scheduled" ? "warning" : "neutral"}>
                {svcStatus === "started" ? "running" : svcStatus === "none" ? "not loaded" : svcStatus ?? "unknown"}
              </Pill>
            </div>
            {#if svc.user}
              <div class="svc-meta text-muted">user: {svc.user}</div>
            {/if}
            <div class="svc-actions">
              <button
                class="svc-btn"
                onclick={() => svcAct("start")}
                disabled={svcPending || svcStatus === "started"}
                title={svcStatus === "started" ? "Already running" : "Start service"}
              >
                <Play size={14} /> Start
              </button>
              <button
                class="svc-btn"
                onclick={() => svcAct("stop")}
                disabled={svcPending || svcStatus === "stopped" || svcStatus === "none"}
                title={svcStatus === "started" ? "Stop service" : "Not running"}
              >
                <Square size={14} /> Stop
              </button>
              <button
                class="svc-btn"
                onclick={() => svcAct("restart")}
                disabled={svcPending}
                title="Restart service"
              >
                <RotateCcw size={14} /> Restart
              </button>
            </div>
          </section>
        {/if}

        {#if detail.caveats}
          <section class="caveats">
            <h3>Caveats</h3>
            <pre>{detail.caveats}</pre>
          </section>
        {/if}

        {#if detail.dependencies.length > 0}
          <section class="collapse">
            <button class="collapse-head" aria-expanded={depsOpen} onclick={() => (depsOpen = !depsOpen)}>
              {#if depsOpen}<ChevronDown size={14} />{:else}<ChevronRight size={14} />{/if}
              <span>Dependencies ({detail.dependencies.length})</span>
            </button>
            {#if depsOpen}
              <ul class="deps">
                {#each detail.dependencies as d (d)}
                  <li>{d}</li>
                {/each}
              </ul>
            {/if}
          </section>
        {/if}

        {#if detail.conflictsWith.length > 0}
          <section class="collapse">
            <button class="collapse-head" aria-expanded={dependentsOpen} onclick={() => (dependentsOpen = !dependentsOpen)}>
              {#if dependentsOpen}<ChevronDown size={14} />{:else}<ChevronRight size={14} />{/if}
              <span>Conflicts with ({detail.conflictsWith.length})</span>
            </button>
            {#if dependentsOpen}
              <ul class="deps">
                {#each detail.conflictsWith as c (c)}<li>{c}</li>{/each}
              </ul>
            {/if}
          </section>
        {/if}
      {/if}
    </div>

    <footer class="actions">
      {#if isInstalled && isOutdated}
        <Button variant="primary" onclick={doUpgrade}>
          {#snippet icon()}<ArrowUpCircle size={16} />{/snippet}
          Upgrade
        </Button>
        <Button variant="danger" onclick={() => (confirmUninstall = true)}>
          {#snippet icon()}<Trash2 size={16} />{/snippet}
          Uninstall
        </Button>
      {:else if isInstalled}
        <Button variant="secondary" onclick={doInstall}>
          {#snippet icon()}<RefreshCcw size={16} />{/snippet}
          Reinstall
        </Button>
        <Button variant="danger" onclick={() => (confirmUninstall = true)}>
          {#snippet icon()}<Trash2 size={16} />{/snippet}
          Uninstall
        </Button>
      {:else if pkg}
        <Button variant="primary" onclick={doInstall}>
          {#snippet icon()}<Download size={16} />{/snippet}
          Install
        </Button>
      {/if}
    </footer>
  </aside>

  <DestructiveConfirm
    open={confirmUninstall}
    title={`Uninstall ${ui.selectedPackage.name}?`}
    confirmLabel="Uninstall"
    onCancel={() => (confirmUninstall = false)}
    onConfirm={doUninstall}
  >
    <p>This will remove <strong>{ui.selectedPackage.name}</strong> from your system.</p>
  </DestructiveConfirm>
{/if}

<style>
  .detail {
    /* Width is driven by ui.detailPaneWidth via the inline style binding below.
       Default falls back to the original 420px so the panel keeps working if
       the var is somehow unset (e.g. SSR or pre-mount). */
    width: var(--detail-pane-width, 420px);
    flex: none;
    background: var(--color-surface-raised);
    border-left: 1px solid var(--color-border);
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
    animation: slideIn var(--motion-duration-base) var(--motion-ease-out);
  }
  @keyframes slideIn {
    from { transform: translateX(8px); opacity: 0; }
    to   { transform: translateX(0); opacity: 1; }
  }
  @media (prefers-reduced-motion: reduce) {
    .detail { animation: none; }
  }

  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: var(--space-4);
    border-bottom: 1px solid var(--color-border);
    gap: var(--space-3);
  }
  .head-left { display: flex; align-items: center; gap: var(--space-2); min-width: 0; }
  h1 { font-size: var(--text-h1); font-weight: var(--fw-semibold); }
  /* h1 receives programmatic focus when the slide-over opens (a11y).
     Suppress its focus ring — the slide-in animation + panel context are the visual cue;
     the ring on a non-interactive heading would be misleading. */
  h1:focus { outline: none; box-shadow: none; }
  .close { color: var(--color-text-muted); padding: 4px; border-radius: var(--radius-sm); }
  .close:hover { background: var(--color-surface-sunken); color: var(--color-text-primary); }

  .body {
    padding: var(--space-4);
    flex: 1;
    overflow-y: auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .meta {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .meta > div { display: grid; grid-template-columns: 80px 1fr; gap: var(--space-2); font-size: var(--text-body-sm); }
  .meta dt { color: var(--color-text-muted); }
  .meta dd { color: var(--color-text-primary); }
  .warn { color: var(--color-warning-strong); margin-left: var(--space-2); font-weight: var(--fw-medium); } /* AA text contrast */
  .icon-source { color: var(--color-text-secondary); }

  /* Category pills sit in the dd column; let them wrap if there are many. */
  .cat-pills {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .cat-pill {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px var(--space-2);
    height: 20px;
    border-radius: var(--radius-full);
    border: 1px solid var(--color-border);
    background: var(--color-surface-sunken);
    color: var(--color-text-secondary);
    font-size: var(--text-caption);
    font-weight: var(--fw-medium);
    line-height: 1;
    cursor: pointer;
    transition: background 0.12s ease, border-color 0.12s ease, color 0.12s ease;
  }
  .cat-pill:hover {
    background: var(--color-brand-subtle);
    border-color: var(--color-brand);
    color: var(--color-text-primary);
  }
  .cat-pill:focus-visible {
    outline: 2px solid var(--color-accent);
    outline-offset: 2px;
  }

  .desc {
    color: var(--color-text-secondary);
    line-height: var(--lh-normal);
    overflow-wrap: anywhere;
    word-break: break-word;
  }

  .homepage {
    display: inline-flex; align-items: center; gap: var(--space-1);
    color: var(--color-text-link);
    font-size: var(--text-body-sm);
    max-width: 100%;
    /* long URLs that exceed the pane wrap rather than clipping at the edge */
    overflow-wrap: anywhere;
    word-break: break-word;
    text-align: left;
  }
  .homepage:hover { text-decoration: underline; }
  /* Allow the URL to use the full available width and wrap; the ellipsis-truncate
     of the old design clipped paths mid-segment at narrow widths. */
  .homepage .truncate {
    max-width: 100%;
    overflow-wrap: anywhere;
    word-break: break-word;
    white-space: normal;
  }

  /* ── Service card (per-package brew services controls) ── */
  .service-card {
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-surface-sunken);
    padding: var(--space-3);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    transition: opacity 0.12s ease;
  }
  .service-card.pending { opacity: 0.6; }
  .svc-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
  }
  .svc-head h3 {
    font-size: var(--text-h3);
    font-weight: var(--fw-semibold);
    margin: 0;
  }
  .svc-meta { font-size: var(--text-body-sm); }
  .svc-actions {
    display: flex;
    gap: var(--space-2);
  }
  .svc-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 4px var(--space-2);
    height: 28px;
    border-radius: var(--radius-sm);
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    color: var(--color-text-secondary);
    font-size: var(--text-body-sm);
    cursor: pointer;
    transition: background 0.12s ease, color 0.12s ease, border-color 0.12s ease;
  }
  .svc-btn:not(:disabled):hover {
    background: var(--color-surface-raised);
    color: var(--color-text-primary);
    border-color: var(--color-accent);
  }
  .svc-btn:disabled { opacity: 0.4; cursor: default; }

  .caveats {
    background: var(--color-warning-subtle);
    color: var(--color-text-primary);
    border-left: 3px solid var(--color-warning);
    padding: var(--space-3) var(--space-4);
    border-radius: var(--radius-md);
    /* Caveats commonly contain long $HOMEBREW_PREFIX paths — wrap aggressively
       on any character so they don't clip at the pane edge. */
    overflow-wrap: anywhere;
    word-break: break-word;
    min-width: 0;
  }
  .caveats h3 { font-size: var(--text-h3); margin-bottom: var(--space-2); color: var(--color-warning-strong); } /* AA text contrast on warning-subtle */
  .caveats pre {
    font-family: var(--font-mono);
    font-size: var(--text-mono);
    white-space: pre-wrap;     /* preserve brew's newlines AND allow wrapping */
    word-break: break-word;
    overflow-wrap: anywhere;
    /* Fallback: if a single unbreakable token (long hash, no whitespace) still
       overflows, give the <pre> its own horizontal scroll instead of clipping
       at the pane edge. */
    overflow-x: auto;
    max-width: 100%;
  }

  .collapse-head {
    display: inline-flex; align-items: center; gap: var(--space-1);
    color: var(--color-text-secondary);
    font-size: var(--text-body-sm);
    font-weight: var(--fw-medium);
    padding: var(--space-1) 0;
  }
  .collapse-head:hover { color: var(--color-text-primary); }
  .deps {
    padding-left: var(--space-4);
    margin-top: var(--space-1);
    display: flex; flex-direction: column; gap: 2px;
    font-size: var(--text-body-sm);
    color: var(--color-text-secondary);
    /* Dependency names with slashes (e.g. "homebrew/cask/foo") can be long;
       wrap them rather than letting the pane scroll horizontally. */
    overflow-wrap: anywhere;
    word-break: break-word;
    min-width: 0;
  }
  .deps li { min-width: 0; }
  .deps li::before { content: "·"; margin-right: var(--space-2); color: var(--color-text-muted); }

  .actions {
    display: flex;
    gap: var(--space-3);
    padding: var(--space-4);
    border-top: 1px solid var(--color-border);
    justify-content: flex-end;
  }

  .error { padding: var(--space-4); display: flex; flex-direction: column; gap: var(--space-3); }

  /* ── GitHub stats card (Phase 12c) ── */
  .gh-card {
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-surface-sunken);
    padding: var(--space-2) var(--space-3);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    font-size: var(--text-body-sm);
  }
  .gh-loading {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--color-text-muted);
  }
  .gh-stats {
    display: inline-flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    color: var(--color-text-primary);
  }
  .gh-stat {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .gh-sep { color: var(--color-text-muted); }
  .gh-muted { color: var(--color-text-muted); }
  .gh-archived {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px;
    background: var(--color-warning-subtle);
    border-radius: var(--radius-sm);
    color: var(--color-warning-strong);
  }
  .gh-license-mismatch {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--color-text-muted);
    font-size: var(--text-caption);
  }
  .gh-license-mismatch code {
    font-family: var(--font-mono);
    font-size: var(--text-mono);
  }
  .gh-error {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--color-text-muted);
    font-size: var(--text-body-sm);
  }
  :global(.spin-slow) {
    animation: spin 1.5s linear infinite;
  }
  @keyframes spin {
    from { transform: rotate(0deg); }
    to   { transform: rotate(360deg); }
  }
  @media (prefers-reduced-motion: reduce) {
    :global(.spin-slow) { animation: none; }
  }
</style>
