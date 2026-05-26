// Static JSON renderer for the brew-browser trending-collector.
//
// Reads the snapshots table and emits:
//   - <out>/index.json                — top-N summary blob
//   - <out>/formula/<name>.json       — per-formula history series
//   - <out>/cask/<name>.json          — per-cask history series

import { join } from "node:path";
import { mkdir } from "node:fs/promises";

import {
  INDEX_TOP_N,
  SPARKLINE_DAYS,
  isSafePackageToken,
  shiftISO,
  velocityIndex,
  writeJsonAtomic,
} from "./common.js";

/** Render every output JSON file from the current DB state. */
export async function renderAll(db, outDir) {
  await mkdir(outDir, { recursive: true });

  const today = latestSnapshotDate(db);
  if (!today) {
    console.warn("[render] no snapshots in DB — nothing to render");
    return { indexEntries: 0, perPackageFiles: 0 };
  }

  // Build a flat per-package payload first; we'll partition into the
  // index summary + per-package files from the same source.
  const packages = collectPackages(db, today);

  // Compute velocity + sparkline for each package; sort by velocity
  // desc for the index leaderboard.
  const enriched = packages
    .map((p) => ({
      ...p,
      velocity: velocityIndex(p.latest30d ?? 0, p.latest90d ?? 0, p.latest365d ?? 0),
      sparkline: buildSparkline(db, p.name, p.kind, today),
    }))
    // Drop packages with no meaningful chart data from the index
    // (per-package files still get written so direct fetches work).
    .filter((p) => p.velocity !== null || p.sparkline.length > 0);

  const indexPackages = enriched
    .filter((p) => p.velocity !== null)
    .sort((a, b) => b.velocity - a.velocity)
    .slice(0, INDEX_TOP_N)
    .map((p) => ({
      name: p.name,
      kind: p.kind,
      velocityIndex: p.velocity,
      sparkline: p.sparkline,
    }));

  const index = {
    generatedAt: new Date().toISOString(),
    packages: indexPackages,
    cacheAgeSeconds: 0,
  };
  await writeJsonAtomic(join(outDir, "index.json"), index);

  // Per-package series files. Refresh existing files; clean up files
  // for packages that have aged out so the on-disk tree doesn't grow
  // unbounded across years.
  let perPackageWritten = 0;
  for (const p of packages) {
    if (!isSafePackageToken(p.name)) continue; // belt-and-suspenders
    const series = buildSeries(db, p.name, p.kind);
    if (series.points.length === 0) continue;
    const path = join(outDir, p.kind, `${p.name}.json`);
    await writeJsonAtomic(path, series);
    perPackageWritten += 1;
  }

  return {
    indexEntries: indexPackages.length,
    perPackageFiles: perPackageWritten,
  };
}

/** Return the most recent `snapshot_date` in the DB. Used as the
 *  reference point for the index payload. */
function latestSnapshotDate(db) {
  const row = db
    .prepare("SELECT MAX(snapshot_date) AS d FROM snapshots")
    .get();
  return row?.d ?? null;
}

/** Pull a flat list of `{ name, kind, latest30d, latest90d, latest365d }`
 *  from the DB at the given reference date. Joins across windows in
 *  SQL so we get a single round-trip. */
function collectPackages(db, referenceDate) {
  // Use `install` for formulae and `cask_install` for casks. Both
  // map onto the same TrendingHistorySeries shape on the frontend.
  const rows = db
    .prepare(
      `
      SELECT package_name AS name, kind, window, count
      FROM snapshots
      WHERE snapshot_date = @date
        AND (
          (kind = 'formula' AND category = 'install')
          OR (kind = 'cask' AND category = 'cask_install')
        )
      `,
    )
    .all({ date: referenceDate });

  // Pivot in JS: { (name, kind) -> { latest30d, latest90d, latest365d } }
  const acc = new Map();
  for (const r of rows) {
    const key = `${r.kind}:${r.name}`;
    if (!acc.has(key)) {
      acc.set(key, { name: r.name, kind: r.kind });
    }
    const slot = acc.get(key);
    if (r.window === "30d") slot.latest30d = r.count;
    if (r.window === "90d") slot.latest90d = r.count;
    if (r.window === "365d") slot.latest365d = r.count;
  }
  return [...acc.values()];
}

/** Build the inline sparkline array for a package — `SPARKLINE_DAYS`
 *  most-recent estimated daily-installs values, padded with 0 when
 *  history doesn't go back that far. Falls back to 30d rolling-window
 *  counts when daily granularity isn't available yet (cold start). */
function buildSparkline(db, name, kind, today) {
  const series = buildSeries(db, name, kind);
  if (series.points.length === 0) return [];
  // Take the last SPARKLINE_DAYS days of `estimatedDailyInstalls` if
  // available, else fall back to count_30d. Trim/pad as needed.
  const tail = series.points.slice(-SPARKLINE_DAYS);
  const vals = tail.map(
    (p) => p.estimatedDailyInstalls ?? p.count30d ?? 0,
  );
  while (vals.length < SPARKLINE_DAYS && vals.length > 0) {
    vals.unshift(0);
  }
  return vals;
}

/** Build the full TrendingHistorySeries for a package. */
function buildSeries(db, name, kind) {
  // Pivot rows from the snapshots table into one point per snapshot
  // date. We carry `count_30d`, `count_90d`, `count_365d`, plus the
  // install-on-request 30d count, plus the per-day estimate when
  // adjacent-day subtraction is meaningful.
  const baseCategory = kind === "formula" ? "install" : "cask_install";

  const rows = db
    .prepare(
      `
      SELECT snapshot_date, category, window, count, source
      FROM snapshots
      WHERE package_name = @name
        AND kind = @kind
        AND (
          category = @baseCategory
          OR category = 'install_on_request'
        )
      ORDER BY snapshot_date ASC
      `,
    )
    .all({ name, kind, baseCategory });

  if (rows.length === 0) {
    return {
      name,
      kind,
      points: [],
      generatedAt: new Date().toISOString(),
      cacheAgeSeconds: 0,
    };
  }

  // Pivot: { date -> { count30d, count90d, count365d, countIor30d, source } }
  const byDate = new Map();
  for (const r of rows) {
    if (!byDate.has(r.snapshot_date)) {
      byDate.set(r.snapshot_date, { source: r.source });
    }
    const slot = byDate.get(r.snapshot_date);
    if (r.category === baseCategory) {
      if (r.window === "30d") slot.count30d = r.count;
      if (r.window === "90d") slot.count90d = r.count;
      if (r.window === "365d") slot.count365d = r.count;
    } else if (r.category === "install_on_request" && r.window === "30d") {
      slot.countIor30d = r.count;
    }
    // Source: if any row for this date is 'daily', treat the whole
    // point as daily; only mark seed when every row is from seed.
    if (r.source === "daily") slot.source = "daily";
  }

  // Compute per-day estimated installs via adjacent-day subtraction
  // of count_30d snapshots. After ~30 days of nightly snapshots this
  // produces clean numbers; before then it's noisy (we don't know
  // what dropped off the 30-day edge), so the estimate is null.
  const dates = [...byDate.keys()].sort();
  for (let i = 1; i < dates.length; i++) {
    const today = byDate.get(dates[i]);
    const yesterday = byDate.get(dates[i - 1]);
    // Only compute when both points are daily snapshots — seed
    // buckets are coarse and the subtraction doesn't make sense.
    if (today.source !== "daily" || yesterday.source !== "daily") continue;
    if (today.count30d == null || yesterday.count30d == null) continue;
    const delta = today.count30d - yesterday.count30d;
    // delta = today's adds minus 31-day-ago drop-offs. After we have
    // 30+ days of history, this is a clean daily install count. Before,
    // it's biased; we still surface it because trajectory > absolute.
    today.estimatedDailyInstalls = Math.max(0, delta);
  }

  const points = dates.map((date) => {
    const slot = byDate.get(date);
    return {
      date,
      count30d: slot.count30d ?? null,
      count90d: slot.count90d ?? null,
      count365d: slot.count365d ?? null,
      countInstallOnRequest30d: slot.countIor30d ?? null,
      estimatedDailyInstalls: slot.estimatedDailyInstalls ?? null,
      source: slot.source,
    };
  });

  return {
    name,
    kind,
    points,
    generatedAt: new Date().toISOString(),
    cacheAgeSeconds: 0,
  };
}
