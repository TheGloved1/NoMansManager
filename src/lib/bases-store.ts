import type { BaseSummary, SaveFileInfo, TypeCounts } from "./types";
import type { BasesSortDir, BasesSortKey } from "./bases-settings";

// In-memory view cache for the Bases page. Module state survives SPA
// navigation (unlike component state) and dies with the app — so returning
// to the page repaints instantly without persistence anywhere on disk.
export interface BasesViewSnapshot {
  saveDir: string | null;
  saveDirManual: boolean;
  saveFiles: SaveFileInfo[];
  selectedSave: string | null;
  loadedSave: string | null;
  bases: BaseSummary[];
  counts: TypeCounts | null;
  selectedBase: number | null;
  search: string;
  filter: "Both" | "Corvettes" | "Planetary";
  sortKey: BasesSortKey;
  sortDir: BasesSortDir;
  /** mtime of the save file when it was loaded, for stale detection */
  loadedMtime: number | null;
}

let cached: BasesViewSnapshot | null = null;

export function saveBasesView(snap: BasesViewSnapshot): void {
  cached = snap;
}

export function takeBasesView(): BasesViewSnapshot | null {
  return cached;
}

export function clearBasesView(): void {
  cached = null;
}
