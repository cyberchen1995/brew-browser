/**
 * Enrichment store (Phase 13) — lazy-loads the bundled `enrichment.json.gz`
 * payload via the `enrichment_data` Tauri command and exposes per-token
 * lookup helpers.
 *
 * **AI Features toggle gate.** Every public lookup checks
 * `settings.effective.aiFeaturesEnabled` and short-circuits to `null`
 * when the user has the toggle off — so UI components don't have to
 * re-implement the gate at every call site.
 *
 * Singleton: import `enrichment` from this module everywhere; the store
 * fetches once per process and caches the result.
 */

import { enrichmentData } from "$lib/api";
import { settings } from "$lib/stores/settings.svelte";
import type { EnrichmentData, EnrichmentEntry } from "$lib/types";

class EnrichmentStore {
  data: EnrichmentData | null = $state(null);
  loading: boolean = $state(false);
  error: string | null = $state(null);

  private loadPromise: Promise<void> | null = null;

  /** Lazy-load on first access. Safe to call repeatedly — only fetches once.
      Failures are recorded on `this.error` but never rethrown; the store
      stays usable with `data === null` and lookups return `null`. */
  async ensureLoaded(): Promise<void> {
    if (this.data || this.loadPromise) {
      return this.loadPromise ?? Promise.resolve();
    }
    this.loading = true;
    this.error = null;
    this.loadPromise = (async () => {
      try {
        this.data = await enrichmentData();
      } catch (e) {
        this.error = `Failed to load enrichment: ${String(e)}`;
      } finally {
        this.loading = false;
        this.loadPromise = null;
      }
    })();
    return this.loadPromise;
  }

  /** Returns the enrichment entry for a token, OR null when:
   *  - AI features toggle is off
   *  - Data not loaded
   *  - No entry for this token (empty placeholder catalog)
   */
  lookup(token: string): EnrichmentEntry | null {
    if (!settings.effective.aiFeaturesEnabled) return null;
    if (!this.data) return null;
    return this.data.entries[token] ?? null;
  }

  /** Friendly-name short-circuit: returns `friendlyName` if available AND
   *  the AI toggle is on; else null. Components that only want the
   *  display-name override can skip the full lookup + null-checks.
   */
  friendlyName(token: string): string | null {
    const e = this.lookup(token);
    if (!e) return null;
    return e.friendlyName ?? null;
  }

  /** Summary short-circuit: returns the AI-generated 1-2 sentence
   *  "what + when" description when AI Features is on AND the bundle
   *  has an entry; else null. Used by list-view Description columns
   *  with `summary > upstream desc > null` fallback semantics.
   */
  summaryOf(token: string): string | null {
    const e = this.lookup(token);
    if (!e) return null;
    return e.summary ?? null;
  }

  /** True when the AI Features toggle is on AND we have data loaded.
   *  Components use this to short-circuit render branches without
   *  re-deriving the gate. */
  get visible(): boolean {
    return settings.effective.aiFeaturesEnabled && this.data !== null;
  }
}

export const enrichment = new EnrichmentStore();
