#!/usr/bin/env node
// Seed the brew-browser trending-history DB with three historical
// "buckets" per package derived from today's rolling 30d/90d/365d
// counts. Run ONCE on the day the collector goes live so the
// per-package charts have something to show before nightly snapshots
// accumulate.
//
// The seed trick:
//   c30          = installs in the last 30 days
//   c90 − c30    = installs in days 31..90 (60 days)
//   c365 − c90   = installs in days 91..365 (275 days)
//
// We synthesize three "snapshot" rows per package per category, dated
// at the midpoint of each bucket, with source='seed' so the renderer
// can distinguish coarse historical from real daily snapshots.

import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  CATEGORIES,
  WINDOWS,
  extractItems,
  fetchAnalyticsPayload,
  openDb,
  parseCount,
  shiftISO,
  todayISO,
} from "./lib/common.js";

const __dirname = dirname(fileURLToPath(import.meta.url));

const DB_PATH = process.env.DB_PATH ?? resolve(__dirname, "state/db.sqlite");

async function main() {
  console.log(`[seed] db=${DB_PATH}`);
  const db = openDb(DB_PATH);

  // Refuse to seed twice — the seed trick is a one-shot bootstrap; if
  // any 'seed' rows already exist this would double-up the historical
  // buckets and confuse the renderer. Operator can DROP and re-run if
  // truly needed.
  const existingSeed = db
    .prepare("SELECT COUNT(*) AS n FROM snapshots WHERE source = 'seed'")
    .get();
  if (existingSeed.n > 0) {
    console.error(
      `[seed] DB already has ${existingSeed.n} seed rows; refusing to re-seed.`,
    );
    console.error(
      "       To reset, drop the snapshots table and rerun: " +
        `sqlite3 ${DB_PATH} 'DROP TABLE snapshots;'`,
    );
    process.exit(2);
  }

  // Pull current rolling-window counts for every category/window.
  const today = todayISO();
  console.log(`[seed] reference date: ${today}`);

  // Map of { category -> { window -> { name -> count } } }
  const counts = new Map();
  for (const cat of CATEGORIES) {
    counts.set(cat, new Map());
    for (const win of WINDOWS) {
      console.log(`[seed]   fetch ${cat} ${win}`);
      try {
        const payload = await fetchAnalyticsPayload(cat, win);
        const items = extractItems(payload);
        const m = new Map();
        for (const item of items) {
          if (!item?.formula) continue;
          m.set(item.formula, parseCount(item.count));
        }
        counts.get(cat).set(win, m);
        console.log(`[seed]     ${m.size} items`);
      } catch (e) {
        console.error(`[seed]   FAILED ${cat} ${win}: ${e.message}`);
      }
    }
  }

  // Synthesize three historical buckets per (package, category) using
  // the rolling-window subtraction. Bucket midpoints:
  //   bucket A — days 0..30,  midpoint = today − 15
  //   bucket B — days 31..90, midpoint = today − 60
  //   bucket C — days 91..365, midpoint = today − 228
  const bucketDates = {
    recent: shiftISO(today, -15),
    mid: shiftISO(today, -60),
    older: shiftISO(today, -228),
  };
  console.log(
    `[seed] bucket midpoints: recent=${bucketDates.recent} ` +
      `mid=${bucketDates.mid} older=${bucketDates.older}`,
  );

  // Use a single transaction for the insert — much faster than N
  // statements and atomic so a crash mid-seed leaves a clean DB.
  // `source` is parameterized so the today-dated rows can be tagged
  // 'daily' directly (they're the kickoff for the real daily series).
  const insert = db.prepare(
    `INSERT INTO snapshots
       (package_name, kind, snapshot_date, category, window, count, source)
     VALUES (@name, @kind, @date, @category, @window, @count, @source)`,
  );

  let inserted = 0;
  const txn = db.transaction(() => {
    for (const cat of CATEGORIES) {
      const c30 = counts.get(cat).get("30d") ?? new Map();
      const c90 = counts.get(cat).get("90d") ?? new Map();
      const c365 = counts.get(cat).get("365d") ?? new Map();

      // Union of names that appear in any window. A package can be
      // in 365d but not 30d (newly inactive); we still want its history.
      const names = new Set([...c30.keys(), ...c90.keys(), ...c365.keys()]);

      const kind = cat === "cask_install" ? "cask" : "formula";

      for (const name of names) {
        const v30 = c30.get(name) ?? 0;
        const v90 = c90.get(name) ?? 0;
        const v365 = c365.get(name) ?? 0;

        // Recent bucket: just c30 itself, dated at midpoint.
        if (v30 > 0) {
          insert.run({
            name,
            kind,
            date: bucketDates.recent,
            category: cat,
            window: "30d",
            count: v30,
            source: "seed",
          });
          inserted += 1;
        }
        // Mid bucket: c90 − c30 (installs in days 31..90).
        const midCount = Math.max(0, v90 - v30);
        if (midCount > 0) {
          insert.run({
            name,
            kind,
            date: bucketDates.mid,
            category: cat,
            window: "30d", // stored in the 30d slot for consistency w/ daily
            count: midCount,
            source: "seed",
          });
          inserted += 1;
        }
        // Older bucket: c365 − c90 (installs in days 91..365).
        const olderCount = Math.max(0, v365 - v90);
        if (olderCount > 0) {
          insert.run({
            name,
            kind,
            date: bucketDates.older,
            category: cat,
            window: "30d",
            count: olderCount,
            source: "seed",
          });
          inserted += 1;
        }

        // Also insert the actual c30/c90/c365 rows as a DAILY snapshot
        // for today. This kicks off the "real daily" history so the
        // next nightly run has a predecessor to subtract from.
        for (const [win, val] of [
          ["30d", v30],
          ["90d", v90],
          ["365d", v365],
        ]) {
          if (val > 0) {
            insert.run({
              name,
              kind,
              date: today,
              category: cat,
              window: win,
              count: val,
              source: "daily",
            });
            inserted += 1;
          }
        }
      }
    }
  });

  txn();

  console.log(`[seed] inserted ${inserted} rows`);

  // Vacuum once at the end of bootstrap for a tidy on-disk file.
  db.exec("VACUUM");
  db.close();

  console.log("[seed] done. run `node collect.js` nightly via cron.");
}

main().catch((e) => {
  console.error("[seed] failed:", e);
  process.exit(1);
});
