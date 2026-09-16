import { LazyStore } from "@tauri-apps/plugin-store";

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
