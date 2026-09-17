import { invoke } from "@tauri-apps/api/core";
import type {
  AppConfig,
  BackupInfo,
  BaseImportResult,
  BaseSummary,
  DecompressResult,
  DeployResult,
  ExportResult,
  ImportResult,
  Mod,
  Profile,
  SaveFileInfo,
} from "./types";

export type {
  BackupInfo,
  BaseImportResult,
  BaseSummary,
  DecompressResult,
  ExportResult,
  SaveFileInfo,
} from "./types";

export interface LogEntry {
  ts: string;
  level: string;
  source: string;
  msg: string;
}

type Source = "mods" | "bases" | "app";
type Detail<T> = string | ((res: T) => string) | undefined;

function shortPath(p: string): string {
  const parts = p.split("/");
  return parts.length > 2 ? "…/" + parts.slice(-2).join("/") : p;
}

// Every backend call is recorded in the activity log (Logs page). Logging
// itself must never break the action it records, and huge payloads/results
// are summarized, never stored.
async function logged<T>(
  source: Source,
  label: string,
  detail: Detail<T>,
  fn: () => Promise<T>,
): Promise<T> {
  try {
    const res = await fn();
    const suffix =
      typeof detail === "function"
        ? detail(res)
        : detail
          ? detail
          : "";
    void invoke("append_log", {
      level: "info",
      source,
      message: suffix ? `${label} (${suffix})` : label,
    }).catch(() => {});
    return res;
  } catch (e) {
    void invoke("append_log", {
      level: "error",
      source,
      message: `${label} — failed: ${e}`,
    }).catch(() => {});
    throw e;
  }
}

export const api = {
  findNmsInstall: (manual?: string | null) =>
    logged("mods", "Find game install", manual ? shortPath(manual) : undefined, () =>
      invoke<string | null>("find_nms_install", { manual }),
    ),
  getModsDir: (gameRoot: string) =>
    logged("mods", "Resolve MODS dir", undefined, () =>
      invoke<string>("get_mods_dir", { gameRoot }),
    ),
  getStoreDir: () =>
    logged("mods", "Resolve store dir", undefined, () => invoke<string>("get_store_dir")),
  scanStore: () =>
    logged("mods", "Scan mod store", (res: Mod[]) => `${res.length} mods`, () =>
      invoke<Mod[]>("scan_store"),
    ),
  scanDeployed: (modsDir: string) =>
    logged(
      "mods",
      "Scan deployed mods",
      (res: Record<string, string>) => `${Object.keys(res).length} deployed`,
      () => invoke<Record<string, string>>("scan_deployed", { modsDir }),
    ),
  importMods: (modsDir: string, doMove?: boolean) =>
    logged(
      "mods",
      `Import mods${doMove ? " (move)" : ""}`,
      (res: ImportResult) => `${res.imported.length} imported, ${res.skipped.length} skipped`,
      () => invoke<ImportResult>("import_mods", { modsDir, doMove }),
    ),
  addMods: (paths: string[]) =>
    logged("mods", `Add ${paths.length} path(s)`, (res: ImportResult) =>
      res.imported.length
        ? res.imported.join(", ")
        : (res.skipped[0] ?? "skipped"),
    () => invoke<ImportResult>("add_mods", { paths }),
    ),
  deployMods: (modsDir: string, orderedIds: string[], deployMode?: string) =>
    logged(
      "mods",
      "Deploy",
      (res: DeployResult) =>
        res.errors.length ? `errors: ${res.errors[0]}` : `${res.deployed} deployed`,
      () => invoke<DeployResult>("deploy_mods", { modsDir, orderedIds, deployMode }),
    ),
  globalDisableEnabled: (modsDir: string) =>
    logged("mods", "Check global disable", undefined, () =>
      invoke<boolean>("global_disable_enabled", { modsDir }),
    ),
  setGlobalDisable: (modsDir: string, disable: boolean) =>
    logged("mods", disable ? "Disable all mods" : "Re-enable mods", undefined, () =>
      invoke<void>("set_global_disable", { modsDir, disable }),
    ),
  listProfiles: () =>
    logged("mods", "List profiles", (res: string[]) => res.join(", "), () =>
      invoke<string[]>("list_profiles"),
    ),
  loadProfile: (name: string) =>
    logged("mods", `Load profile '${name}'`, undefined, () =>
      invoke<Profile>("load_profile", { name }),
    ),
  saveProfile: (profile: Profile) =>
    logged("mods", `Save profile '${profile.name}'`, undefined, () =>
      invoke<void>("save_profile", { profile }),
    ),
  createProfile: (name: string, cloneFrom?: string | null) =>
    logged(
      "mods",
      `Create profile '${name}'`,
      cloneFrom ? `cloned from '${cloneFrom}'` : undefined,
      () => invoke<Profile>("create_profile", { name, cloneFrom }),
    ),
  deleteProfile: (name: string) =>
    logged("mods", `Delete profile '${name}'`, undefined, () =>
      invoke<void>("delete_profile", { name }),
    ),
  renameProfile: (old: string, newName: string) =>
    logged("mods", `Rename profile '${old}' → '${newName}'`, undefined, () =>
      invoke<void>("rename_profile", { old, new: newName }),
    ),
  loadConfig: () =>
    logged("app", "Load config", undefined, () => invoke<AppConfig>("load_config")),
  saveConfig: (config: AppConfig) =>
    logged("app", "Save config", undefined, () =>
      invoke<void>("save_config", { config }),
    ),
  canSymlink: () =>
    logged("app", "Check symlink support", undefined, () => invoke<boolean>("can_symlink")),
  removeStoreMod: (id: string) =>
    logged("mods", `Remove mod '${id}'`, undefined, () =>
      invoke<void>("remove_store_mod", { id }),
    ),
  renameStoreMod: (id: string, newName: string) =>
    logged("mods", `Rename mod '${id}' → '${newName}'`, undefined, () =>
      invoke<string>("rename_store_mod", { id, newName }),
    ),
  openFolder: (path: string) =>
    logged("app", "Open folder", shortPath(path), () =>
      invoke<void>("open_folder", { path }),
    ),
  // --- Bases (save editing) ---
  findSaveDirs: () =>
    logged("bases", "Find save folders", (res: string[]) => `${res.length} found`, () =>
      invoke<string[]>("find_save_dirs"),
    ),
  findSaveDir: (prefer?: string | null) =>
    logged(
      "bases",
      "Autodetect save folder",
      prefer ? shortPath(prefer) : undefined,
      () => invoke<string | null>("find_save_dir", { prefer }),
    ),
  listSaveFiles: (saveDir: string) =>
    logged(
      "bases",
      "List saves",
      (res: SaveFileInfo[]) => res.map((f) => f.name).join(", "),
      () => invoke<SaveFileInfo[]>("list_save_files", { saveDir }),
    ),
  listSaveSubdirs: (saveDir: string) =>
    logged("bases", "List save subfolders", undefined, () =>
      invoke<string[]>("list_save_subdirs", { saveDir }),
    ),
  decompressSave: (saveDir: string, saveFile: string) =>
    logged(
      "bases",
      `Decompress '${saveFile}'`,
      (res: DecompressResult) => `${res.bases.length} bases`,
      () => invoke<DecompressResult>("decompress_save", { saveDir, saveFile }),
    ),
  listBases: (filter?: string | null) =>
    logged(
      "bases",
      "List bases",
      (res: BaseSummary[]) => `${res.length} shown`,
      () => invoke<BaseSummary[]>("list_bases", { filter }),
    ),
  exportBase: (idx: number, outPath?: string | null) =>
    logged("bases", `Export base (slot ${idx})`, outPath ? shortPath(outPath) : "JSON", () =>
      invoke<ExportResult>("export_base", { idx, outPath }),
    ),
  exportNmsbase: (idx: number, outPath?: string | null) =>
    logged("bases", `Export NMSBASE (slot ${idx})`, outPath ? shortPath(outPath) : undefined, () =>
      invoke<ExportResult>("export_nmsbase", { idx, outPath }),
    ),
  getBaseJson: (idx: number) =>
    logged("bases", `View base (slot ${idx})`, undefined, () =>
      invoke<string>("get_base_json", { idx }),
    ),
  getNmsbaseText: (idx: number) =>
    logged("bases", `Copy NMSBASE (slot ${idx})`, undefined, () =>
      invoke<string>("get_nmsbase_text", { idx }),
    ),
  readSaveTextFile: (path: string) =>
    logged("bases", "Read import file", shortPath(path), () =>
      invoke<string>("read_text_file", { path }),
    ),
  importBase: (idx: number, payload: string) =>
    logged(
      "bases",
      `Inject into slot ${idx}`,
      (res: BaseImportResult) => `${res.objects} objects`,
      () => invoke<BaseImportResult>("import_base", { idx, payload }),
    ),
  recompressSave: (mode: string) =>
    logged("bases", mode === "overwrite" ? "Recompress (overwrite live)" : "Recompress to output", undefined, () =>
      invoke<string>("recompress_save", { mode }),
    ),
  backupSaves: (saveDir: string) =>
    logged("bases", "Back up saves", (res: string[]) => `${res.length} file(s)`, () =>
      invoke<string[]>("backup_saves", { saveDir }),
    ),
  listBackups: (stem?: string | null) =>
    logged(
      "bases",
      "List backups",
      (res: BackupInfo[]) => `${res.length} found`,
      () => invoke<BackupInfo[]>("list_backups", { stem }),
    ),
  restoreSave: (backupPath: string, saveDir: string, saveFile: string) =>
    logged("bases", `Restore '${saveFile}'`, undefined, () =>
      invoke<string>("restore_save", { backupPath, saveDir, saveFile }),
    ),
  // --- Activity log itself (never self-logs) ---
  readLogs: () => invoke<LogEntry[]>("read_logs"),
  clearLogs: () => invoke<void>("clear_logs"),
  getLogsDir: () => invoke<string>("get_logs_dir"),
};
