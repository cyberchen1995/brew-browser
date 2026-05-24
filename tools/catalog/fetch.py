#!/usr/bin/env python3
"""
fetch.py — Bake a fresh Homebrew catalog snapshot into the app bundle.

When to run:
    Pre-release, or any time you want to refresh the catalog that ships
    inside the app binary (the in-app Refresh button writes to user-data
    instead and never touches these committed files).

What it produces:
    src-tauri/data/catalog/formula.json.gz   (~6-8 MB gzipped)
    src-tauri/data/catalog/cask.json.gz      (~2-3 MB gzipped)
    src-tauri/data/catalog/manifest.json     {as_of, counts, sizes, source}

The Rust backend `include_bytes!`s the two .gz files (and the manifest)
at compile time via `src-tauri/src/catalog/mod.rs`. There is no runtime
file dependency on this script.

Stdlib only — no `requests`, no `pip install` step. Runs anywhere Python
3.9+ is available.

Usage:
    python3 tools/catalog/fetch.py

Exit codes:
    0  both files fetched, gzipped, and manifest written
    1  network or file error (no partial writes — temp + rename pattern)
"""

from __future__ import annotations

import gzip
import json
import sys
import time
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

# Source endpoints — Homebrew's published JSON catalog.
API_BASE = "https://formulae.brew.sh/api/"
FORMULA_URL = API_BASE + "formula.json"
CASK_URL = API_BASE + "cask.json"

# Output paths, relative to the repo root.
REPO_ROOT = Path(__file__).resolve().parents[2]
OUTPUT_DIR = REPO_ROOT / "src-tauri" / "data" / "catalog"
FORMULA_GZ = OUTPUT_DIR / "formula.json.gz"
CASK_GZ = OUTPUT_DIR / "cask.json.gz"
MANIFEST = OUTPUT_DIR / "manifest.json"

# Hard cap on download size — same as Rust's MAX_CATALOG_BYTES (64 MiB).
# Current catalog is ~30 MB formulae + ~15 MB casks raw; 64 MiB is ample.
MAX_RAW_BYTES = 64 * 1024 * 1024

USER_AGENT = "brew-browser-catalog-fetch/0.1 (+https://github.com/msitarzewski/brew-browser)"


def fetch(url: str) -> bytes:
    """GET `url` and return the raw bytes. Caps at MAX_RAW_BYTES so a
    pathological response can't OOM the build host."""
    print(f"  GET {url}", flush=True)
    t0 = time.monotonic()
    req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(req, timeout=60) as resp:
        if resp.status != 200:
            raise RuntimeError(f"HTTP {resp.status} from {url}")
        # Read in chunks so we can enforce the size cap.
        chunks: list[bytes] = []
        total = 0
        while True:
            chunk = resp.read(1024 * 1024)
            if not chunk:
                break
            total += len(chunk)
            if total > MAX_RAW_BYTES:
                raise RuntimeError(
                    f"{url}: response exceeded {MAX_RAW_BYTES} bytes; refusing to continue"
                )
            chunks.append(chunk)
    elapsed = time.monotonic() - t0
    print(f"    {total:,} bytes in {elapsed:.1f}s", flush=True)
    return b"".join(chunks)


def write_gz_atomic(path: Path, raw: bytes) -> int:
    """gzip-compress `raw` and write to `path` via a temp file + rename.
    Returns the compressed byte count."""
    tmp = path.with_suffix(path.suffix + ".tmp")
    compressed = gzip.compress(raw, compresslevel=9)
    tmp.write_bytes(compressed)
    tmp.replace(path)
    return len(compressed)


def main() -> int:
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    print(f"Catalog fetch -> {OUTPUT_DIR}")
    print("Fetching formula.json…", flush=True)
    formula_raw = fetch(FORMULA_URL)
    print("Fetching cask.json…", flush=True)
    cask_raw = fetch(CASK_URL)

    # Parse just enough to count entries — we don't reshape; the Rust
    # side knows the structure.
    formulae = json.loads(formula_raw)
    casks = json.loads(cask_raw)
    if not isinstance(formulae, list) or not isinstance(casks, list):
        print("ERROR: unexpected JSON shape (expected top-level array)", file=sys.stderr)
        return 1

    formula_count = len(formulae)
    cask_count = len(casks)

    print("Compressing…", flush=True)
    formula_compressed = write_gz_atomic(FORMULA_GZ, formula_raw)
    cask_compressed = write_gz_atomic(CASK_GZ, cask_raw)

    manifest = {
        "as_of": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "formula_count": formula_count,
        "cask_count": cask_count,
        "formula_compressed_bytes": formula_compressed,
        "cask_compressed_bytes": cask_compressed,
        "fetched_from": API_BASE,
    }
    tmp = MANIFEST.with_suffix(".json.tmp")
    tmp.write_text(json.dumps(manifest, indent=2) + "\n")
    tmp.replace(MANIFEST)

    total_compressed = formula_compressed + cask_compressed
    print()
    print(f"  formulae: {formula_count:>6}  ({formula_compressed:>10,} bytes gzipped)")
    print(f"  casks:    {cask_count:>6}  ({cask_compressed:>10,} bytes gzipped)")
    print(f"  total compressed: {total_compressed:,} bytes ({total_compressed / 1024 / 1024:.2f} MiB)")
    print(f"  manifest: {MANIFEST}")
    print(f"  as_of:    {manifest['as_of']}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (urllib.error.URLError, RuntimeError) as e:
        print(f"ERROR: {e}", file=sys.stderr)
        sys.exit(1)
