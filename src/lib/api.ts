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

export const api = {
  findNmsInstall: (manual?: string | null) =>
    invoke<string | null>("find_nms_install", { manual }),
  getModsDir: (gameRoot: string) =>
    invoke<string>("get_mods_dir", { gameRoot }),
  getStoreDir: () => invoke<string>("get_store_dir"),
  scanStore: () => invoke<Mod[]>("scan_store"),
  scanDeployed: (modsDir: string) =>
    invoke<Record<string, string>>("scan_deployed", { modsDir }),
  importMods: (modsDir: string, doMove?: boolean) =>
    invoke<ImportResult>("import_mods", { modsDir, doMove }),
  addMods: (paths: string[]) => invoke<ImportResult>("add_mods", { paths }),
  deployMods: (modsDir: string, orderedIds: string[], deployMode?: string) =>
    invoke<DeployResult>("deploy_mods", { modsDir, orderedIds, deployMode }),
  globalDisableEnabled: (modsDir: string) =>
    invoke<boolean>("global_disable_enabled", { modsDir }),
  setGlobalDisable: (modsDir: string, disable: boolean) =>
    invoke<void>("set_global_disable", { modsDir, disable }),
  listProfiles: () => invoke<string[]>("list_profiles"),
  loadProfile: (name: string) => invoke<Profile>("load_profile", { name }),
  saveProfile: (profile: Profile) => invoke<void>("save_profile", { profile }),
  createProfile: (name: string, cloneFrom?: string | null) =>
    invoke<Profile>("create_profile", { name, cloneFrom }),
  deleteProfile: (name: string) => invoke<void>("delete_profile", { name }),
  renameProfile: (old: string, newName: string) =>
    invoke<void>("rename_profile", { old, new: newName }),
  loadConfig: () => invoke<AppConfig>("load_config"),
  saveConfig: (config: AppConfig) => invoke<void>("save_config", { config }),
  canSymlink: () => invoke<boolean>("can_symlink"),
  removeStoreMod: (id: string) => invoke<void>("remove_store_mod", { id }),
  renameStoreMod: (id: string, newName: string) =>
    invoke<string>("rename_store_mod", { id, newName }),
  openFolder: (path: string) => invoke<void>("open_folder", { path }),
  // --- Bases (save editing) ---
  findSaveDirs: () => invoke<string[]>("find_save_dirs"),
  findSaveDir: (prefer?: string | null) =>
    invoke<string | null>("find_save_dir", { prefer }),
  listSaveFiles: (saveDir: string) =>
    invoke<SaveFileInfo[]>("list_save_files", { saveDir }),
  listSaveSubdirs: (saveDir: string) =>
    invoke<string[]>("list_save_subdirs", { saveDir }),
  decompressSave: (saveDir: string, saveFile: string) =>
    invoke<DecompressResult>("decompress_save", { saveDir, saveFile }),
  listBases: (filter?: string | null) =>
    invoke<BaseSummary[]>("list_bases", { filter }),
  exportBase: (idx: number, outPath?: string | null) =>
    invoke<ExportResult>("export_base", { idx, outPath }),
  exportNmsbase: (idx: number, outPath?: string | null) =>
    invoke<ExportResult>("export_nmsbase", { idx, outPath }),
  getBaseJson: (idx: number) => invoke<string>("get_base_json", { idx }),
  getNmsbaseText: (idx: number) => invoke<string>("get_nmsbase_text", { idx }),
  readSaveTextFile: (path: string) => invoke<string>("read_text_file", { path }),
  importBase: (idx: number, payload: string) =>
    invoke<BaseImportResult>("import_base", { idx, payload }),
  recompressSave: (mode: string) => invoke<string>("recompress_save", { mode }),
  backupSaves: (saveDir: string) =>
    invoke<string[]>("backup_saves", { saveDir }),
  listBackups: (stem?: string | null) =>
    invoke<BackupInfo[]>("list_backups", { stem }),
  restoreSave: (backupPath: string, saveDir: string, saveFile: string) =>
    invoke<string>("restore_save", { backupPath, saveDir, saveFile }),
};
