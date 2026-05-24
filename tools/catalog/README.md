# tools/catalog/ — Bundled Homebrew catalog snapshot

This directory builds the catalog snapshot that ships baked into the app
binary. The app's in-app **Refresh** button fetches a fresh copy at
runtime and writes it under `~/Library/Application Support/brew-browser/catalog/`;
that user-data copy supersedes the bundled one. The bundled snapshot is
the offline / first-launch fallback.

## When to run

- Before tagging a release (so the shipped app starts with a reasonably
  fresh catalog)
- Any time the upstream API shape changes and Rust deserialisation needs
  a fresh fixture to verify against
- Otherwise: never — daily refresh is not the goal, that's the in-app
  button's job

## What it produces

Three files in `src-tauri/data/catalog/`:

| File | Size | What |
|------|------|------|
| `formula.json.gz` | ~6–8 MB | gzip -9 of `https://formulae.brew.sh/api/formula.json` |
| `cask.json.gz`    | ~2–3 MB | gzip -9 of `https://formulae.brew.sh/api/cask.json` |
| `manifest.json`   | <1 KB   | `{as_of, formula_count, cask_count, *_compressed_bytes, fetched_from}` |

Total uncompressed payload: ~45 MB. Compressed: ~10 MB. These three
files are read by Rust at compile time via `include_bytes!` /
`include_str!` (see `src-tauri/src/catalog/mod.rs`).

## Usage

```
python3 tools/catalog/fetch.py
```

Stdlib only — no `pip install` step. Python 3.9+ required.

The script uses temp + atomic rename, so an interrupted run leaves the
prior snapshot intact.
