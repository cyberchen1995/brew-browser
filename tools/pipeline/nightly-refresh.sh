#!/usr/bin/env bash
# Nightly regeneration of brew-browser's AI categories + descriptions, and the
# static tree served for the app's opt-in *live* updates.
#
# Transparent + committed. This script hard-codes NO private hostnames or paths
# — everything host-specific comes from env vars. Deploy guide: see README.md.
#
#   REPO_DIR     path to the brew-browser clone   (default: the repo this lives in)
#   OUT_DIR      served output dir (render target) (default: $REPO_DIR/tools/pipeline/out)
#   DATA_BRANCH  branch the regenerated data is force-pushed to (default: data/auto-refresh)
#   PYTHON       python interpreter               (default: $REPO_DIR/.venv/bin/python)
#   ENRICH_FLAGS enrich.py tier flags             (default: --tier-a)
#
# Idempotent + no-ops cleanly when nothing changed. Exits non-zero on hard error.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="${REPO_DIR:-$(cd "$SCRIPT_DIR/../.." && pwd)}"
DATA_BRANCH="${DATA_BRANCH:-data/auto-refresh}"
PYTHON="${PYTHON:-$REPO_DIR/.venv/bin/python}"
ENRICH_FLAGS="${ENRICH_FLAGS:---tier-a}"
export OUT_DIR="${OUT_DIR:-$REPO_DIR/tools/pipeline/out}"

cd "$REPO_DIR"
log() { printf '%s %s\n' "$(date -u +%FT%TZ)" "$*"; }

log "=== nightly-refresh start (repo=$REPO_DIR out=$OUT_DIR branch=$DATA_BRANCH) ==="

# 1. Sync regeneration logic from main (keep tools current). Reset is safe:
#    runtime state (tools/*/state, .venv, .env) is gitignored + untracked, so
#    a hard reset of tracked files leaves the incremental-diff state intact.
git fetch --quiet origin
git checkout --quiet main
git reset --hard --quiet origin/main

# 2. Regenerate data — each step is incremental (diff-aware via its own state/).
log "catalog fetch…";          "$PYTHON" tools/catalog/fetch.py
log "categorize…";             "$PYTHON" tools/categorize/categorize.py
log "enrich ($ENRICH_FLAGS)…"; "$PYTHON" tools/enrich/enrich.py $ENRICH_FLAGS

# 3. Render the served tree from the FRESH data (before the reset in step 4).
log "render served…"
"$PYTHON" tools/pipeline/render_served.py

# 4. Force-push the data delta to DATA_BRANCH (= current main + latest data, so
#    it's always cleanly mergeable at release), then reset local main clean so
#    the next run fast-forwards. The data commit lives only on the remote branch.
git add src-tauri/data
if git diff --cached --quiet; then
  log "no data changes — skipping commit"
else
  git -c user.name="brew-browser-bot" -c user.email="bot@users.noreply.github.com" \
      commit --quiet -m "chore(data): nightly categorize+enrich refresh $(date -u +%F)"
  git push --force --quiet origin "HEAD:$DATA_BRANCH"
  log "pushed data delta -> $DATA_BRANCH"
  git reset --hard --quiet origin/main
fi

log "=== nightly-refresh done ==="
