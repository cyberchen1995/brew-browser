# landing

Static landing page for brew-browser, served at `brew-browser.zerologic.com` from umbp via Caddy.

## Files

- `index.html` — the page
- `style.css` — embedded design tokens matching the app (dark-first, warm amber, OKLCH)
- `brew-browser.svg` — the app icon (copy of `../docs/icon/brew-browser.svg`)

## Deploy to umbp

From this directory:

```sh
rsync -avz --delete \
  --exclude README.md \
  ./ michael@umbp:Sites/brew-browser/
```

Caddy config on umbp is managed manually.

## Update flow

1. Edit `index.html` / `style.css` locally
2. View locally: `python3 -m http.server -d . 8089` then open `http://localhost:8089`
3. `rsync` to umbp when ready
