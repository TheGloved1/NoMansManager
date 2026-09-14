import { invoke } from "@tauri-apps/api/core";
import type {
  AppConfig,
  DeployResult,
  ImportResult,
  Mod,
  Profile,
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
  openFolder: (path: string) => invoke<void>("open_folder", { path }),
};
