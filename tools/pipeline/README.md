# brew-browser — nightly data pipeline

Build-time tooling. Regenerates the AI **categories** + **descriptions** nightly
and renders the static tree the app fetches for **opt-in live updates**.

This never runs from inside the app. It runs on a build host, on a schedule.

## What it does (`nightly-refresh.sh`)

1. `git reset --hard origin/main` — sync the regeneration logic.
2. `tools/catalog/fetch.py` — refresh the Homebrew catalog.
3. `tools/categorize/categorize.py` — incremental category pass → `src-tauri/data/categories.json`.
4. `tools/enrich/enrich.py --tier-a` — incremental description pass → `src-tauri/data/enrichment.json.gz`.
5. `tools/pipeline/render_served.py` — render `$OUT_DIR/{version.json, categories.json, entry/<token>.json}` for live fetch.
6. Force-push the data delta to `data/auto-refresh` (always "current main + latest data"; merge it when cutting a release so the *bundled* baseline stays current too).

All three LLM/catalog steps are diff-aware (their own `state/`), so a typical
night hits the API for only the handful of packages that changed.

## Configuration (env vars — no private host/paths in this repo)

| Var | Default | Meaning |
|-----|---------|---------|
| `REPO_DIR` | the repo this script lives in | brew-browser clone |
| `OUT_DIR` | `$REPO_DIR/tools/pipeline/out` | served render target (point at the web-served dir) |
| `DATA_BRANCH` | `data/auto-refresh` | branch the regenerated data is force-pushed to |
| `PYTHON` | `$REPO_DIR/.venv/bin/python` | interpreter |
| `ENRICH_FLAGS` | `--tier-a` | enrich tiers to run |

## Deploy (generic — substitute your own host/paths)

```sh
# On the build host, as the deploy user:
git clone https://github.com/msitarzewski/brew-browser.git "$REPO_DIR"
cd "$REPO_DIR"
python3 -m venv .venv
.venv/bin/pip install -r tools/categorize/requirements.txt -r tools/enrich/requirements.txt

# Anthropic key — one key serves categorize + enrich. .env is gitignored.
cp tools/enrich/.env.example tools/enrich/.env   # then paste ANTHROPIC_API_KEY
cp tools/categorize/.env.example tools/categorize/.env  # same key

# First run (full bulk — minutes, a few $). Subsequent runs are incremental.
OUT_DIR="$OUT_DIR" .venv/bin/... # see nightly-refresh.sh; or just:
OUT_DIR="$OUT_DIR" REPO_DIR="$REPO_DIR" tools/pipeline/nightly-refresh.sh
```

### Cron (run after the trending collector settles)

```cron
30 3 * * * REPO_DIR="$REPO_DIR" OUT_DIR="$OUT_DIR" "$REPO_DIR/tools/pipeline/nightly-refresh.sh" >> "$LOGFILE" 2>&1
```

### Caddy (serve `$OUT_DIR` at `…/enrichment/*`)

Mirror the trending-history block (6h cache, IP-redacted logs — see
`memory-bank/security.md`):

```caddy
handle_path /enrichment/* {
    root * {$OUT_DIR}
    file_server
    @writes method POST PUT DELETE PATCH
    respond @writes 405
    header {
        Cache-Control "public, max-age=21600"
        -Set-Cookie
        -Server
    }
}
```

The app fetches `https://<public-domain>/enrichment/{version.json,categories.json,entry/<token>.json}`
only when the user opts in (Settings → live category/description updates) and
Offline Mode is off.
