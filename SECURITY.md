# Security Policy

## Supported Versions

Only the latest release is supported with security updates.

## Reporting a Vulnerability

Please report vulnerabilities privately via a GitHub security advisory or by
opening an issue at <https://github.com/TheGloved1/NoMansManager/issues>.
Do not open a public PR with exploit details.

## Security Notes

- Updates are signed (Tauri updater + `updater.json` signatures). The public key
  ships in `src-tauri/tauri.conf.json`; the private key lives only in the
  `TAURI_SIGNING_PRIVATE_KEY` GitHub Secret / local env var and is never committed.
- The frontend has no direct filesystem or opener access
  (`src-tauri/capabilities/default.json` grants only `core`, `dialog`
  open/save, `store`, `updater`, and window focus/show). All file operations run
  in the Rust backend with managed-path validation (game `MODS` dir, save dirs,
  app data/backups/logs).
- The Tauri CSP is intentionally `null`: the UI is fully local SvelteKit output
  with no remote content, and SvelteKit requires inline scripts. There is no
  remote-script attack surface; network access is limited to the GitHub Releases
  updater endpoint.
