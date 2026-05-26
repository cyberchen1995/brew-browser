<script lang="ts">
  import { onMount } from "svelte";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import TrendingUp from "@lucide/svelte/icons/trending-up";

  import Button from "./Button.svelte";
  import Pill from "./Pill.svelte";
  import LoadingState from "./LoadingState.svelte";
  import EmptyState from "./EmptyState.svelte";
  import SortableHeader from "./SortableHeader.svelte";
  import { trending } from "$lib/stores/trending.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { packages } from "$lib/stores/packages.svelte";
  import { enrichment } from "$lib/stores/enrichment.svelte";
  import { catalog } from "$lib/stores/catalog.svelte";
  import type { TrendingEntry, TrendingWindow } from "$lib/types";

  onMount(() => {
    if (!trending.report) trending.load();
    // Prime the enrichment store so AI-enriched friendly names start
    // resolving as soon as data lands.
    enrichment.ensureLoaded();
    // Description + version columns read from the catalog's per-token
    // maps — prime them so the columns light up as soon as data lands.
    void catalog.ensureSummariesLoaded();
  });

  /** AI-enriched friendly name for a token, or null. Inline call from
   *  row markup; sync Map.get under the hood. */
  function friendlyOf(token: string): string | null {
    return enrichment.friendlyName(token);
  }

  const windows: TrendingWindow[] = ["30d", "90d", "365d"];

  type SortKey = "rank" | "name" | "kind" | "installs";
  let sortKey: SortKey = $state("rank");
  let sortDir: "asc" | "desc" = $state("asc");

  function changeSort(key: string) {
    const k = key as SortKey;
    if (sortKey === k) {
      sortDir = sortDir === "asc" ? "desc" : "asc";
    } else {
      sortKey = k;
      // Numeric/rank-like keys default to descending on first click (most-installed first)
      sortDir = k === "installs" ? "desc" : "asc";
    }
  }

  let sortedEntries = $derived.by<TrendingEntry[]>(() => {
    if (!trending.report) return [];
    const arr = [...trending.report.entries];
    const mul = sortDir === "asc" ? 1 : -1;
    arr.sort((a, b) => {
      let cmp = 0;
      switch (sortKey) {
        case "rank":     cmp = a.rank - b.rank; break;
        case "name":     cmp = a.name.localeCompare(b.name); break;
        case "kind":     cmp = a.kind.localeCompare(b.kind); break;
        case "installs": cmp = a.installCount - b.installCount; break;
      }
      return cmp * mul;
    });
    return arr;
  });

  let agoLabel = $derived.by(() => {
    if (!trending.report) return "";
    const secs = trending.report.cacheAgeSeconds;
    if (secs < 60) return `Updated ${secs}s ago`;
    const mins = Math.floor(secs / 60);
    if (mins < 60) return `Updated ${mins} min ago`;
    const hrs = Math.floor(mins / 60);
    return `Updated ${hrs}h ago`;
  });

  function openEntry(name: string, kind: "formula" | "cask") {
    ui.selectPackage(name, kind);
  }
</script>

<section class="trending">
  <!-- Pane title ("Trending") moved to the window title bar; head keeps
       the time-window pills, last-updated label, and Refresh. -->
  <header class="panel-head" data-tauri-drag-region>
    <div class="head-right" data-tauri-drag-region="false">
      <div class="pillgroup" role="tablist" aria-label="Time window">
        {#each windows as w (w)}
          <button class:on={trending.window === w} onclick={() => trending.setWindow(w)} role="tab" aria-selected={trending.window === w}>{w}</button>
        {/each}
      </div>
      <span class="ago text-muted">{agoLabel}</span>
      <span class="refresh-wrap">
        <Button size="sm" variant="ghost" onclick={() => trending.load(true)} title="Refresh (⌘R)" ariaLabel="Refresh trending">
          {#snippet icon()}<RefreshCw size={14} />{/snippet}
          Refresh
        </Button>
      </span>
    </div>
  </header>

  <div class="list-wrap">
    {#if trending.loading && !trending.report}
      <LoadingState rows={10} label="Fetching install counts from formulae.brew.sh…" />
    {:else if trending.error}
      <EmptyState title="Couldn't reach formulae.brew.sh" body={trending.error}>
        {#snippet icon()}<TrendingUp size={48} />{/snippet}
        {#snippet cta()}<Button variant="secondary" onclick={() => trending.load(true)}>Retry</Button>{/snippet}
      </EmptyState>
    {:else if trending.report && trending.report.entries.length === 0}
      <EmptyState title="Quiet for now." body="formulae.brew.sh returned no entries for this window.">
        {#snippet icon()}<TrendingUp size={48} />{/snippet}
      </EmptyState>
    {:else if trending.report}
      <div class="list-header" role="row">
        <SortableHeader label="#" sortKey="rank" active={sortKey === "rank"} dir={sortDir} onSort={changeSort} />
        <SortableHeader label="Name" sortKey="name" active={sortKey === "name"} dir={sortDir} onSort={changeSort} />
        <span class="header-desc">Description</span>
        <span class="header-version">Version</span>
        <SortableHeader label="Type" sortKey="kind" active={sortKey === "kind"} dir={sortDir} onSort={changeSort} />
        <SortableHeader label="Installs" sortKey="installs" active={sortKey === "installs"} dir={sortDir} onSort={changeSort} align="right" />
        <span></span>
      </div>
      <ul class="list" aria-label="Trending packages">
        {#each sortedEntries as e (e.name + e.kind)}
          {@const installed = e.installedLocally || packages.isInstalled(e.name, e.kind)}
          {@const isSelected = ui.selectedPackage?.name === e.name && ui.selectedPackage?.kind === e.kind}
          <li>
            <button
              class="row"
              class:selected={isSelected}
              aria-current={isSelected ? "true" : undefined}
              onclick={() => openEntry(e.name, e.kind)}
            >
              <span class="rank">{e.rank}</span>
              <span class="name truncate">
                <span class="name-text">{e.name}</span>
                {#if friendlyOf(e.name)}
                  <span class="friendly-subtitle">{friendlyOf(e.name)}</span>
                {/if}
              </span>
              <span class="desc truncate text-muted">{enrichment.summaryOf(e.name) ?? catalog.descOf(e.name, e.kind) ?? ""}</span>
              <span class="version truncate text-muted">{catalog.versionOf(e.name, e.kind) ?? ""}</span>
              <span class="kind"><Pill tone={e.kind === "formula" ? "formula" : "cask"}>{e.kind}</Pill></span>
              <span class="count mono">{e.installCountFormatted}</span>
              <span class="trail">
                {#if installed}<Pill tone="success">installed</Pill>{/if}
              </span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</section>

<style>
  .trending { display: flex; flex-direction: column; min-height: 0; height: 100%; }
  .panel-head {
    display: flex; justify-content: flex-end; align-items: center;
    padding: var(--space-4);
    border-bottom: 1px solid var(--color-border);
    gap: var(--space-3);
  }
  .head-right { display: flex; align-items: center; gap: var(--space-3); margin-left: auto; }
  .ago { font-size: var(--text-body-sm); white-space: nowrap; }

  /* Narrow-window responsive: drop the "Updated Ns ago" text and the
     Refresh button when the head-right cluster starts to crowd the
     panel (typically when the detail panel is open + the window is
     narrow). Refresh stays available via Cmd+R. */
  @media (max-width: 1000px) {
    .ago { display: none; }
    .refresh-wrap { display: none; }
  }

  /* Trending row has 7 cells (# / NAME / DESC / VERSION / TYPE /
     COUNT / TRAIL). Drop columns in priority order from widest-but-
     least-essential first:
       <= 1100px: drop Trail (7th, installed pill)
       <=  900px: also drop Description (3rd)
       <=  720px: also drop Version (4th); leave # / NAME / TYPE / COUNT. */
  @media (max-width: 1100px) {
    .list-header,
    .row {
      grid-template-columns: 48px minmax(0, 1fr) minmax(0, 2fr) 100px 80px 120px;
    }
    .list-header > :nth-child(7),
    .row > :nth-child(7) { display: none; }
  }
  @media (max-width: 900px) {
    .list-header,
    .row {
      grid-template-columns: 48px minmax(0, 1fr) 100px 80px 120px;
    }
    .list-header > :nth-child(3),
    .list-header > :nth-child(7),
    .row > :nth-child(3),
    .row > :nth-child(7) { display: none; }
  }
  @media (max-width: 720px) {
    .list-header,
    .row {
      grid-template-columns: 48px minmax(0, 1fr) 80px 120px;
    }
    .list-header > :nth-child(3),
    .list-header > :nth-child(4),
    .list-header > :nth-child(7),
    .row > :nth-child(3),
    .row > :nth-child(4),
    .row > :nth-child(7) { display: none; }
  }

  /* Sidebar theme-group pattern: sunken background, no border,
     raised + shadow active state. */
  .pillgroup {
    display: inline-flex;
    background: var(--color-surface-sunken);
    border-radius: var(--radius-md);
    padding: 2px;
    gap: 2px;
  }
  .pillgroup button {
    padding: var(--space-1) var(--space-3);
    border-radius: var(--radius-sm);
    color: var(--color-text-secondary);
    font-size: var(--text-body-sm);
    font-weight: var(--fw-medium);
  }
  .pillgroup button.on {
    background: var(--color-surface-raised);
    color: var(--color-text-primary);
    box-shadow: var(--shadow-xs);
  }

  .list-wrap { flex: 1; overflow-y: auto; min-height: 0; }
  .list-header {
    display: grid;
    grid-template-columns: 48px minmax(0, 1fr) minmax(0, 2fr) 100px 80px 120px 100px;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-3);
    background: var(--color-surface);
    border-bottom: 1px solid var(--color-border);
    position: sticky;
    top: 0;
    z-index: 1;
    /* Prevent column-header text bleeding across cells when the panel is
       narrow (detail panel open + small window). Each cell clips to its
       own column. */
    overflow: hidden;
  }
  .list-header > * { min-width: 0; overflow: hidden; }
  .row > * { min-width: 0; overflow: hidden; }
  .list { display: flex; flex-direction: column; }
  .row {
    display: grid;
    grid-template-columns: 48px minmax(0, 1fr) minmax(0, 2fr) 100px 80px 120px 100px;
    align-items: center;
    gap: var(--space-3);
    width: 100%;
    padding: var(--space-2) var(--space-3);
    min-height: 32px;
    text-align: left;
    color: var(--color-text-primary);
    font-size: var(--text-body);
    border-bottom: 1px solid var(--color-border);
  }
  .row:hover { background: var(--color-surface-sunken); }
  .row.selected {
    background: var(--color-selection-strong);
    color: var(--color-text-inverse);
  }
  .row.selected .rank,
  .row.selected .count { color: inherit; }
  .row.selected .desc,
  .row.selected .version { color: inherit; opacity: 0.85; }
  .row.selected .friendly-subtitle {
    color: var(--color-text-inverse);
    opacity: 0.75;
  }
  .desc {
    font-size: var(--text-body-sm);
    color: var(--color-text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .version {
    font-size: var(--text-body-sm);
    color: var(--color-text-secondary);
    font-variant-numeric: tabular-nums;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .header-desc,
  .header-version {
    font-size: var(--text-body-sm);
    font-weight: var(--fw-medium);
    color: var(--color-text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }
  .rank { color: var(--color-text-muted); font-variant-numeric: tabular-nums; }
  /* Vertical flex container so the optional AI-enriched friendly_name
     subtitle (Phase 13) stacks below the raw token. Children manage
     their own truncation. */
  .name {
    font-weight: var(--fw-medium);
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    min-width: 0;
    white-space: normal;
  }
  .name-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }
  .friendly-subtitle {
    display: block;
    font-size: var(--text-caption);
    color: var(--color-text-muted);
    font-weight: var(--fw-regular, 400);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
    line-height: 1.2;
    margin-top: 1px;
  }
  .count { font-variant-numeric: tabular-nums; text-align: right; color: var(--color-text-secondary); }
  .trail { display: flex; justify-content: flex-end; }
</style>
