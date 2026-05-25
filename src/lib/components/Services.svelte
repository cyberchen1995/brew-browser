<script lang="ts">
  import { onMount } from "svelte";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import Play from "@lucide/svelte/icons/play";
  import Square from "@lucide/svelte/icons/square";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import Activity from "@lucide/svelte/icons/activity";

  import Button from "./Button.svelte";
  import Pill from "./Pill.svelte";
  import LoadingState from "./LoadingState.svelte";
  import EmptyState from "./EmptyState.svelte";
  import SortableHeader from "./SortableHeader.svelte";
  import { services } from "$lib/stores/services.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { packages } from "$lib/stores/packages.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { normalizeServiceStatus, type Service, type ServiceStatus } from "$lib/types";
  import { reportableToastError } from "$lib/util/reportIssue";

  type SortKey = "name" | "status" | "user";
  let sortKey: SortKey = $state("status");
  let sortDir: "asc" | "desc" = $state("asc");

  onMount(() => {
    services.load();
  });

  function changeSort(key: string) {
    const k = key as SortKey;
    if (sortKey === k) {
      sortDir = sortDir === "asc" ? "desc" : "asc";
    } else {
      sortKey = k;
      sortDir = "asc";
    }
  }

  /** Status sort order: started → scheduled → error → stopped → none → unknown. */
  const STATUS_ORDER: Record<ServiceStatus, number> = {
    started:   0,
    scheduled: 1,
    error:     2,
    stopped:   3,
    none:      4,
    unknown:   5,
  };

  let sorted = $derived.by<Service[]>(() => {
    const arr = [...services.list];
    const mul = sortDir === "asc" ? 1 : -1;
    arr.sort((a, b) => {
      let cmp = 0;
      switch (sortKey) {
        case "name":
          cmp = a.name.localeCompare(b.name);
          break;
        case "status":
          cmp = STATUS_ORDER[normalizeServiceStatus(a.status)] -
                STATUS_ORDER[normalizeServiceStatus(b.status)];
          if (cmp === 0) cmp = a.name.localeCompare(b.name);
          break;
        case "user":
          cmp = (a.user ?? "").localeCompare(b.user ?? "");
          if (cmp === 0) cmp = a.name.localeCompare(b.name);
          break;
      }
      return cmp * mul;
    });
    return arr;
  });

  function pillTone(s: ServiceStatus): "success" | "warning" | "danger" | "neutral" {
    switch (s) {
      case "started":   return "success";
      case "scheduled": return "warning";
      case "error":     return "danger";
      default:          return "neutral";
    }
  }

  function statusLabel(s: ServiceStatus): string {
    switch (s) {
      case "started":   return "running";
      case "stopped":   return "stopped";
      case "none":      return "not loaded";
      case "error":     return "error";
      case "scheduled": return "scheduled";
      case "unknown":   return "unknown";
    }
  }

  async function act(name: string, action: "start" | "stop" | "restart") {
    try {
      await services.act(name, action);
      toast.success(`${action.charAt(0).toUpperCase() + action.slice(1)}ed ${name}`);
    } catch (e) {
      reportableToastError(`Failed to ${action} ${name}`, e);
    }
  }

  function openPackage(name: string) {
    // Services are formulae by definition.
    if (packages.isInstalled(name, "formula")) {
      ui.selectPackage(name, "formula");
    }
  }
</script>

<section class="services">
  <!-- Pane title ("Services") moved to the window title bar; head keeps
       the running/total count and Refresh. -->
  <header class="panel-head" data-tauri-drag-region>
    <div class="head-right" data-tauri-drag-region="false">
      <span class="text-muted count">
        {#if services.list.length > 0}
          {services.list.filter((s) => normalizeServiceStatus(s.status) === "started").length} running ·
          {services.list.length} total
        {/if}
      </span>
      <span class="refresh-wrap">
        <Button size="sm" variant="ghost" onclick={() => services.load(true)} ariaLabel="Refresh services" title="Refresh (⌘R)" disabled={services.loading}>
          {#snippet icon()}<RefreshCw size={14} />{/snippet}
          Refresh
        </Button>
      </span>
    </div>
  </header>

  <div class="list-wrap">
    {#if services.loading && services.list.length === 0}
      <LoadingState rows={6} label="Loading brew services…" />
    {:else if services.error}
      <EmptyState title="Couldn't load services" body={services.error}>
        {#snippet icon()}<Activity size={48} />{/snippet}
        {#snippet cta()}
          <Button variant="secondary" onclick={() => services.load(true)}>Retry</Button>
        {/snippet}
      </EmptyState>
    {:else if services.list.length === 0}
      <EmptyState
        title="No background services."
        body="Install something like postgresql, redis, or nginx and they'll show up here."
      >
        {#snippet icon()}<Activity size={48} />{/snippet}
      </EmptyState>
    {:else}
      <div class="list-header" role="row">
        <SortableHeader label="Name" sortKey="name" active={sortKey === "name"} dir={sortDir} onSort={changeSort} />
        <SortableHeader label="Status" sortKey="status" active={sortKey === "status"} dir={sortDir} onSort={changeSort} />
        <SortableHeader label="User" sortKey="user" active={sortKey === "user"} dir={sortDir} onSort={changeSort} />
        <span class="header-actions">Actions</span>
      </div>
      <ul class="list" aria-label="Brew services">
        {#each sorted as s (s.name)}
          {@const ns = normalizeServiceStatus(s.status)}
          {@const isPending = services.isPending(s.name)}
          {@const isSelected = ui.selectedPackage?.name === s.name && ui.selectedPackage?.kind === "formula"}
          <li>
            <div class="row" class:pending={isPending} class:selected={isSelected}>
              <button
                class="name truncate"
                aria-current={isSelected ? "true" : undefined}
                onclick={() => openPackage(s.name)}
                title={`Open ${s.name} in detail`}
              >
                {s.name}
              </button>
              <span class="status">
                <Pill tone={pillTone(ns)}>{statusLabel(ns)}</Pill>
              </span>
              <span class="user truncate text-muted">{s.user ?? "—"}</span>
              <div class="actions">
                <button
                  class="act"
                  onclick={() => act(s.name, "start")}
                  disabled={isPending || ns === "started"}
                  title={ns === "started" ? "Already running" : "Start service"}
                  aria-label={`Start ${s.name}`}
                >
                  <Play size={14} />
                </button>
                <button
                  class="act"
                  onclick={() => act(s.name, "stop")}
                  disabled={isPending || ns === "stopped" || ns === "none"}
                  title={ns === "started" ? "Stop service" : "Not running"}
                  aria-label={`Stop ${s.name}`}
                >
                  <Square size={14} />
                </button>
                <button
                  class="act"
                  onclick={() => act(s.name, "restart")}
                  disabled={isPending}
                  title="Restart service"
                  aria-label={`Restart ${s.name}`}
                >
                  <RotateCcw size={14} />
                </button>
              </div>
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</section>

<style>
  .services { display: flex; flex-direction: column; min-height: 0; height: 100%; }
  .panel-head {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    padding: var(--space-4);
    border-bottom: 1px solid var(--color-border);
    gap: var(--space-3);
  }
  .head-right { display: flex; align-items: center; gap: var(--space-3); margin-left: auto; }
  .count { font-size: var(--text-body-sm); white-space: nowrap; }

  /* Narrow-window responsive: same pattern as Trending + Library. Drop the
     "N running · M total" count and the Refresh button when the head-right
     cluster crowds the panel. Cmd+R still refreshes the services list. */
  @media (max-width: 1000px) {
    .count { display: none; }
    .refresh-wrap { display: none; }
  }

  .list-wrap { flex: 1; overflow-y: auto; min-height: 0; }
  .list-header {
    display: grid;
    grid-template-columns: minmax(0, 1.5fr) 110px minmax(0, 1fr) 120px;
    gap: var(--space-3);
    padding: var(--space-2) var(--space-4);
    background: var(--color-surface);
    border-bottom: 1px solid var(--color-border);
    position: sticky;
    top: 0;
    z-index: 1;
  }
  .header-actions {
    color: var(--color-text-muted);
    font-size: var(--text-caption);
    font-weight: var(--fw-semibold);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    text-align: right;
  }
  .list { display: flex; flex-direction: column; }
  .row {
    display: grid;
    grid-template-columns: minmax(0, 1.5fr) 110px minmax(0, 1fr) 120px;
    gap: var(--space-3);
    align-items: center;
    padding: var(--space-2) var(--space-4);
    border-bottom: 1px solid var(--color-border);
    transition: opacity 0.12s ease;
  }
  .row.pending { opacity: 0.6; }
  .row:hover { background: var(--color-surface-sunken); }
  .row.selected {
    background: var(--color-selection-strong);
    color: var(--color-text-inverse);
  }
  .row.selected .name,
  .row.selected .user { color: inherit; }
  .name {
    font-weight: var(--fw-medium);
    color: var(--color-text-primary);
    text-align: left;
    background: transparent;
    padding: 0;
    cursor: pointer;
  }
  .name:hover { color: var(--color-text-link); text-decoration: underline; }
  .user { font-size: var(--text-body-sm); }

  .actions {
    display: inline-flex;
    justify-content: flex-end;
    gap: 4px;
  }
  .act {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 26px;
    border-radius: var(--radius-sm);
    background: var(--color-surface-sunken);
    color: var(--color-text-secondary);
    border: 1px solid var(--color-border);
    cursor: pointer;
    transition: background 0.12s ease, color 0.12s ease;
  }
  .act:not(:disabled):hover {
    background: var(--color-surface);
    color: var(--color-text-primary);
    border-color: var(--color-accent);
  }
  .act:disabled { opacity: 0.35; cursor: default; }
</style>
