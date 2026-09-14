# NoModsSky — No Man's Sky Mod Manager

Manage your No Man's Sky mods without the hassle. Finds your game, lets
you install mods, turn them on and off, reorder them, and keep different
profiles.

## What it does

- **Finds your game automatically** — checks your Steam libraries. If it
  can't find it, pick the folder in Settings.
- **Add mods** — drop `.pak` files, folders, or `.zip` archives (even zips
  containing a `.pak` or a folder) onto the window, or use **Add files /
  Add folder**. Bulk add supported — auto-detects zip contents and skips
  duplicates.
- **Import existing mods** — pulls in anything already in `GAMEDATA/MODS`
  with one click (**Import** → Copy or Move).
- **Enable / disable & reorder** — click to select (Ctrl/Cmd to add, Shift
  for range, click empty space or Esc to clear), double-click to toggle,
  drag with the blue line to place before/after, or use **Auto** to sort by
  type and name. Bottom in list wins when mods overlap.
- **Search & filter** — filter by Enabled/Disabled and search by name.
- **One-click Deploy / Purge** — **Deploy** applies enabled mods in order
  (symlinks on Linux, copies on Windows if needed). **Purge** clears deployed
  mods from `GAMEDATA/MODS`.
- **Profiles** — keep separate mod sets for different saves. Create,
  duplicate, rename, delete, and switch on the Profiles page.
- **Customization** — change theme (Default, Rose Pine, Rose Pine
  Moon/Dawn, Catppuccin Mocha/Macchiato/Frappe/Latte) and font (Inter,
  Geist, Space Grotesk, etc.) in Settings. Applies instantly.
- **Quick actions** — open your `GAMEDATA/MODS` or store folder, and
  toggle all mods off via `DISABLEMODS.txt`.

## Download

Get the latest release for your system from
[**Releases**](https://github.com/TheGloved1/NoModsSky/releases) on GitHub:

- **Windows** — `.msi` or `.exe`
- **Linux** — `.deb` or `.AppImage`
- **macOS** — `.dmg`

Just download, install, and launch. No setup.

## How to use

1. **Launch** — it shows your game path if found. If not, go to
   **Settings → Game** and choose your `No Man's Sky` folder.
2. **Add or Import** — drag `.pak`/`.zip`/folders onto the window, or
   click **Add files / Add folder**, or **Import** to bring in mods already in
   the game folder.
3. **Select & reorder** — click to select, `Ctrl`/`Cmd` or `Shift` for
   multi, drag with the line to reorder, **Auto** to sort, double-click to
   toggle a mod.
4. **Deploy** — click **Deploy** to apply the enabled mods. Close the game first.
5. **Profiles** — open **Profiles** to create or switch sets. Each profile
   remembers order and enabled states.

## Where are my mods?

- **Store** — `~/.local/share/nms-mod-manager/mods` (Linux) or the
  equivalent app data folder on Windows/macOS. This is where NoModsSky keeps
  your mods.
- **Game mods folder** — `.../No Man's Sky/GAMEDATA/MODS` — this is what
  NoModsSky deploys to.

## Need help?

- Close No Man's Sky before deploying. Steam Cloud can overwrite —
  disable briefly or keep a backup.
- If a mod doesn't show up, check that it's enabled and that all mods
  aren't disabled (global toggle).
- Drop a `.zip` that contains a single `.pak` or folder? NoModsSky handles
  it automatically.

## License

MIT
