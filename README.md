<!-- <div align="center">
  <img src="static/nms_logo.svg" alt="NoMansManager Banner" width="100%">
</div> -->

<br/>

<div align="center">
  <img src="src-tauri/icons/icon.png" alt="NoMansManager Logo" width="96" height="96">
</div>

<h1 align="center">NoMansManager</h1>

<p align="center">
  <strong>A simple cross-platform No Man's Sky mod manager</strong>
</p>

<p align="center">
  <a href="#features">Features</a> •
  <a href="#how-its-different">How It's Different</a> •
  <a href="#getting-started">Getting Started</a> •
  <a href="#tech-stack">Tech Stack</a> •
  <a href="#license">License</a>
</p>

---

## Features

- **Automatic game detection** — Checks your Steam libraries for No Man's Sky. If it can't find it, a startup dialog (or Settings → Game) lets you pick the folder yourself.
- **Add mods your way** — Drop `.pak` files, folders, or `.zip` archives onto the window, or use **Add files / Add folder**. Bulk add supported — zip contents are auto-detected (even zips containing a single `.pak` or folder) and duplicates are skipped.
- **Import existing mods** — Pulls in anything already in `GAMEDATA/MODS` with one click (**Import** → Copy or Move).
- **Enable / disable & reorder** — Click to select (`Ctrl`/`Cmd` to add, `Shift` for range, click empty space or `Esc` to clear), double-click to toggle, drag with the placement line to reorder, or use **Auto** to sort by type and name. Bottom in list wins when mods overlap.
- **Search & filter** — Filter by Enabled/Disabled and search by name.
- **Profiles** — Keep separate mod sets for different saves. Create, duplicate, rename, delete, and switch on the Profiles page.
- **Customization** — Change theme (Default, Rose Pine, Rose Pine Moon/Dawn, Catppuccin Mocha/Macchiato/Frappe/Latte) and font (Inter, Geist, Space Grotesk, etc.) in Settings. Applies instantly.

## Extras

### Cross-platform

Available for **Windows** (MSI + NSIS), **Linux** (deb + AppImage + rpm), and **macOS** (DMG + app bundle).

---

## Getting Started

### Prerequisites

- Windows, Linux, or macOS

### Installation

1. Download the latest installer for your platform from [Releases](https://github.com/TheGloved1/NoMansManager/releases) or [Downloads](https://gloved.dev/nomodssky/download)
<!-- TODO(phase 3): point Downloads at gloved.dev/nomansmanager/download once the site rename lands -->
2. Run the installer
3. Launch NoMansManager — if your game isn't found automatically, pick your `No Man's Sky` folder when prompted

### Building from Source

```bash
bun install
bun run sync-version
bun tauri build
```

Requires [Bun](https://bun.sh/) and [Rust](https://www.rust-lang.org/).

---

## Tech Stack

| Layer             | Technology                                      |
| ----------------- | ----------------------------------------------- |
| Desktop Framework | [Tauri](https://v2.tauri.app/) (Rust backend)   |
| Frontend          | SvelteKit + Svelte 5 + TypeScript               |
| UI Components     | [shadcn-svelte](https://www.shadcn-svelte.com/) |
| Styling           | Tailwind CSS v4                                 |
| State             | Tauri Store plugin + Svelte stores              |
| Build Tool        | Vite                                            |
| Package Manager   | Bun                                             |
| License           | MIT                                             |

---

## License

- MIT — NoMansManager is free and open-source software
