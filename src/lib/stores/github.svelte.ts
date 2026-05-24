/**
 * GitHub integration store (Phase 12c + 12e).
 *
 * Mirrors backend `github::*` commands into the renderer:
 *
 * - `status` — sign-in summary from the Keychain (no token here, ever).
 * - `repoStatsCache` — per-homepage memo so repeat-opening the same
 *   PackageDetail doesn't re-hit even the backend's 24h disk cache.
 * - `signIn()` — runs the full Device Flow loop: start → poll until
 *   approved/denied/expired. Polling honours the server-provided
 *   `interval` and doubles on `slowDown` per RFC 8628 §3.5.
 *
 * **No token state on the frontend.** Everything authed-sensitive is
 * derived from `status.signedIn`. The actual token lives in the
 * macOS Keychain and is read server-side by the IPC commands.
 */

import {
  githubRepoStats,
  githubSigninPoll,
  githubSigninStart,
  githubSignout,
  githubStatus,
} from "$lib/api";
import {
  isBrewError,
  type DeviceFlowStart,
  type GithubStatus,
  type RepoStats,
} from "$lib/types";

/** Per-row outcome we cache in the frontend. */
export type RepoStatsOutcome =
  | { kind: "loading" }
  | { kind: "loaded"; stats: RepoStats }
  /** Backend returned `null` — homepage isn't GitHub, settings off,
      or 404. UI shows nothing. */
  | { kind: "miss" }
  /** Anonymous 60/hr rate limit hit. UI suggests sign-in. */
  | { kind: "rateLimited"; resetAt: number }
  /** Paranoid mode is on. */
  | { kind: "blocked" }
  /** Other backend error. */
  | { kind: "error"; message: string };

/**
 * Public state of an in-flight Device Flow session. The modal
 * subscribes to this to render the user code + polling spinner.
 */
export type SigninState =
  | { kind: "idle" }
  | { kind: "starting" }
  | {
      kind: "waiting";
      userCode: string;
      verificationUri: string;
      deviceCode: string;
      expiresAt: number;
      intervalMs: number;
    }
  | { kind: "approved" }
  | { kind: "denied" }
  | { kind: "expired" }
  | { kind: "error"; message: string };

class GithubStore {
  /** Last-known status from `githubStatus()`. Null until first load. */
  status: GithubStatus | null = $state(null);

  /** True while a `loadStatus()` is in flight. UI can disable the
      sign-in button. */
  statusLoading: boolean = $state(false);

  /** Per-homepage cache. Using a Map so reactive reads can iterate;
      Svelte 5 reactivity tracks the assignment to `repoStatsCache`. */
  repoStatsCache: Map<string, RepoStatsOutcome> = $state(new Map());

  /** Current Device Flow session. The modal renders based on this. */
  signinState: SigninState = $state({ kind: "idle" });

  /** Per-session AbortController for the polling loop so `cancelSignin`
      can stop the in-flight loop without racing the modal close. */
  private pollAborter: AbortController | null = null;

  /** Read the latest sign-in status from the backend. Idempotent. */
  async loadStatus(): Promise<void> {
    this.statusLoading = true;
    try {
      this.status = await githubStatus();
    } catch (e) {
      // Status read shouldn't fail under normal conditions; if it
      // does we keep the previous value so the UI doesn't flap.
      // (Keychain unavailable is the realistic case.)
      if (isBrewError(e) && e.code === "keychain_unavailable") {
        this.status = { signedIn: false, username: null, scopes: [] };
      }
    } finally {
      this.statusLoading = false;
    }
  }

  /**
   * Get cached stats for `homepage` (kicks off a fetch on first call).
   * Returns the current outcome synchronously; subsequent reads to
   * `repoStatsCache.get(homepage)` will see updates as they arrive.
   */
  async getRepoStats(homepage: string): Promise<RepoStatsOutcome> {
    const existing = this.repoStatsCache.get(homepage);
    if (existing && existing.kind !== "loading") {
      return existing;
    }
    if (existing?.kind === "loading") {
      // Already in flight; let the original call's update propagate.
      return existing;
    }

    // Mark loading and trigger the fetch. Svelte's reactivity tracks
    // the Map assignment.
    const next = new Map(this.repoStatsCache);
    next.set(homepage, { kind: "loading" });
    this.repoStatsCache = next;

    try {
      const stats = await githubRepoStats(homepage);
      const outcome: RepoStatsOutcome = stats
        ? { kind: "loaded", stats }
        : { kind: "miss" };
      const after = new Map(this.repoStatsCache);
      after.set(homepage, outcome);
      this.repoStatsCache = after;
      return outcome;
    } catch (e) {
      let outcome: RepoStatsOutcome;
      if (isBrewError(e)) {
        if (e.code === "github_rate_limited") {
          outcome = { kind: "rateLimited", resetAt: e.resetAt };
        } else if (e.code === "paranoid_mode_blocked") {
          outcome = { kind: "blocked" };
        } else {
          outcome = { kind: "error", message: e.code };
        }
      } else {
        outcome = { kind: "error", message: String(e) };
      }
      const after = new Map(this.repoStatsCache);
      after.set(homepage, outcome);
      this.repoStatsCache = after;
      return outcome;
    }
  }

  /**
   * Run the full Device Flow sign-in: start → poll until terminal.
   * Returns when the session is settled (approved / denied / expired /
   * error). The modal watches `signinState` for progress.
   */
  async signIn(): Promise<void> {
    if (this.signinState.kind === "waiting" || this.signinState.kind === "starting") {
      return; // already in flight
    }
    this.signinState = { kind: "starting" };
    let start: DeviceFlowStart;
    try {
      start = await githubSigninStart();
    } catch (e) {
      this.signinState = {
        kind: "error",
        message: isBrewError(e) ? e.code : String(e),
      };
      return;
    }

    const expiresAt = Date.now() + start.expiresIn * 1000;
    let intervalMs = Math.max(start.interval, 5) * 1000;
    this.signinState = {
      kind: "waiting",
      userCode: start.userCode,
      verificationUri: start.verificationUri,
      deviceCode: start.deviceCode,
      expiresAt,
      intervalMs,
    };

    // Loop. Bounded by expiresAt and by cancellation.
    this.pollAborter = new AbortController();
    const aborter = this.pollAborter;

    while (true) {
      if (aborter.signal.aborted) return;
      if (Date.now() > expiresAt) {
        this.signinState = { kind: "expired" };
        return;
      }
      // Wait one interval. We poll AFTER the wait so the very first
      // call respects the server's recommended cadence; GitHub returns
      // `authorization_pending` immediately if you hit them too fast.
      await sleep(intervalMs, aborter.signal);
      if (aborter.signal.aborted) return;

      let result;
      try {
        result = await githubSigninPoll(start.deviceCode);
      } catch (e) {
        this.signinState = {
          kind: "error",
          message: isBrewError(e) ? e.code : String(e),
        };
        return;
      }

      if (result.kind === "approved") {
        this.signinState = { kind: "approved" };
        // Refresh status so the Settings panel shows the new username.
        await this.loadStatus();
        return;
      }
      if (result.kind === "denied") {
        this.signinState = { kind: "denied" };
        return;
      }
      if (result.kind === "expired") {
        this.signinState = { kind: "expired" };
        return;
      }
      if (result.kind === "slowDown") {
        // RFC 8628 §3.5 — double the interval, capped at 60s.
        intervalMs = Math.min(intervalMs * 2, 60_000);
        if (this.signinState.kind === "waiting") {
          this.signinState = { ...this.signinState, intervalMs };
        }
        continue;
      }
      // pending — keep polling.
    }
  }

  /** User clicked Cancel on the Device Flow modal. Aborts the poll
      loop and resets the session state. The backend has no concept
      of "abort sign-in" — the device_code just expires naturally on
      GitHub's side. */
  cancelSignin(): void {
    this.pollAborter?.abort();
    this.pollAborter = null;
    this.signinState = { kind: "idle" };
  }

  /** Sign out: delete Keychain credentials, refresh status. */
  async signOut(): Promise<void> {
    try {
      await githubSignout();
    } finally {
      // Always refresh — even if delete partially failed, status
      // reflects what's actually stored.
      await this.loadStatus();
    }
    // Drop cached stats so a future request gets the (now anonymous)
    // budget-limited response instead of a stale signed-in result.
    this.repoStatsCache = new Map();
  }
}

/** Module-level singleton. Components import { github } and read `$state`. */
export const github = new GithubStore();

// ---------- Helpers ----------

/** Promise-based sleep that resolves early on AbortSignal. */
function sleep(ms: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve) => {
    const t = setTimeout(resolve, ms);
    signal.addEventListener(
      "abort",
      () => {
        clearTimeout(t);
        resolve();
      },
      { once: true },
    );
  });
}
