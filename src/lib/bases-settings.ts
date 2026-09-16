import { LazyStore } from "@tauri-apps/plugin-store";
import {
  loadTableSort,
  saveTableSort,
  type SortDir,
  type TableSort,
} from "./table-sort";

// Save-folder override for the Bases page. Stored in the shared
// settings.json under its own key so it never interferes with the
// main AppConfig shape.
const store = new LazyStore("settings.json");

export async function loadSaveDirOverride(): Promise<string | null> {
  try {
    return (await store.get<string>("bases_save_dir")) ?? null;
  } catch {
    return null;
  }
}

export async function saveSaveDirOverride(dir: string): Promise<void> {
  await store.set("bases_save_dir", dir);
  await store.save();
}

export async function clearSaveDirOverride(): Promise<void> {
  await store.delete("bases_save_dir");
  await store.save();
}

export type BasesSortKey = "name" | "objects" | "type";
export type BasesSortDir = SortDir;
export type BasesSort = TableSort<BasesSortKey>;

const BASES_SORT_KEY = "bases_sort";
const DEFAULT_SORT: BasesSort = { key: "name", dir: "asc" };
const VALID_KEYS = ["name", "objects", "type"] as const;

export async function loadBasesSort(): Promise<BasesSort> {
  return loadTableSort(BASES_SORT_KEY, VALID_KEYS, DEFAULT_SORT);
}

export async function saveBasesSort(sort: BasesSort): Promise<void> {
  return saveTableSort(BASES_SORT_KEY, sort);
}
