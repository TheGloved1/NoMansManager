use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

mod saves;

// --- App dirs (renamed nms-mod-manager -> nomansmanager; first run migrates) ---
const LEGACY_DATA_DIR_NAME: &str = "nms-mod-manager";
const DATA_DIR_NAME: &str = "nomansmanager";

fn app_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(DATA_DIR_NAME)
}
fn app_config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(DATA_DIR_NAME)
}
fn store_dir() -> PathBuf {
    app_data_dir().join("mods")
}
fn profiles_dir() -> PathBuf {
    app_data_dir().join("profiles")
}
fn config_file() -> PathBuf {
    app_config_dir().join("settings.json")
}
fn ensure_dirs() {
    for p in [
        app_data_dir(),
        app_config_dir(),
        dirs::data_dir()
            .map(|p| p.join(DATA_DIR_NAME))
            .unwrap_or_else(|| PathBuf::from(".")),
        store_dir(),
        profiles_dir(),
    ] {
        let _ = fs::create_dir_all(&p);
    }
    // also state dir linux ~/.local/state -> dirs::data_local_dir alternative not needed, keep simple
}

/// Carry user data (mods, profiles, settings) across the
/// `nms-mod-manager` -> `nomansmanager` rename. Runs on every startup so a
/// partial or skipped first run self-heals later. Rules, per location:
/// - mods/profiles entries: copy anything missing on the new side.
/// - profiles/*.json: if the new copy exists but has an empty mod list while
///   the old one doesn't, the new copy is/regenerated junk — back it up and
///   restore the old one.
/// - settings.json (legacy file + Tauri plugin store): copy only when the new
///   side has no file yet, so we never clobber settings the user already made
///   in the renamed app.
/// Once every byte of an old location is confirmed present on the new side,
/// the old location is removed. Anything unverified is kept, with a log line.
fn migrate_legacy_data_dir() {
    let data_base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    let cfg_base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));


    // content dirs: merge missing entries
    for (old_sub, new_sub) in [
        ("mods", store_dir()),
        ("profiles", profiles_dir()),
    ] {
        let old = data_base.join(LEGACY_DATA_DIR_NAME).join(old_sub);
        if old.is_dir() {
            if merge_missing_entries(&old, &new_sub) {
                eprintln!("migrated {old_sub} -> {}", new_sub.display());
            }
        }
    }
    // profile JSONs: repair regenerated-empty copies from the old ones
    let old_profiles = data_base.join(LEGACY_DATA_DIR_NAME).join("profiles");
    if old_profiles.is_dir() {
        repair_emptied_profiles(&old_profiles, &profiles_dir());
    }
    // settings files: fill only when absent
    let pairs = [
        (
            cfg_base.join(LEGACY_DATA_DIR_NAME).join("settings.json"),
            config_file(),
        ),
        (
            data_base.join("dev.gloved.nomodssky").join("settings.json"),
            data_base
                .join("dev.gloved.nomansmanager")
                .join("settings.json"),
        ),
    ];
    for (old_file, new_file) in pairs {
        if old_file.is_file() && !new_file.exists() {
            if let Some(parent) = new_file.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if fs::copy(&old_file, &new_file).is_ok() {
                eprintln!("migrated settings {}", new_file.display());
            }
        }
    }

    // verified cleanup: remove old locations only once every byte is confirmed
    // present on the new side. Anything unverified stays put, with a log line.
    let old_data = data_base.join(LEGACY_DATA_DIR_NAME);
    if old_data.is_dir() {
        if trees_identical(&old_data, &app_data_dir()) {
            match fs::remove_dir_all(&old_data) {
                Ok(_) => eprintln!("migration: removed {}", old_data.display()),
                Err(e) => eprintln!("migration: could not remove {}: {}", old_data.display(), e),
            }
        } else {
            eprintln!(
                "migration: keeping {} (not fully mirrored yet)",
                old_data.display()
            );
        }
    }
    let old_cfg_file = cfg_base.join(LEGACY_DATA_DIR_NAME).join("settings.json");
    if old_cfg_file.is_file() {
        let new_cfg_file = config_file();
        let same = fs::read(&old_cfg_file)
            .ok()
            .zip(fs::read(&new_cfg_file).ok())
            .map(|(a, b)| a == b)
            .unwrap_or(false);
        if same {
            let _ = fs::remove_file(&old_cfg_file);
            let old_cfg_dir = cfg_base.join(LEGACY_DATA_DIR_NAME);
            // drop the dir too if nothing else lives in it
            let empty = fs::read_dir(&old_cfg_dir)
                .map(|mut e| e.next().is_none())
                .unwrap_or(false);
            if empty {
                let _ = fs::remove_dir(&old_cfg_dir);
            }
            eprintln!("migration: removed {}", old_cfg_file.display());
        }
    }
    let old_plugin_dir = data_base.join("dev.gloved.nomodssky");
    let new_plugin_settings = data_base
        .join("dev.gloved.nomansmanager")
        .join("settings.json");
    if old_plugin_dir.is_dir() {
        let same = fs::read(old_plugin_dir.join("settings.json"))
            .ok()
            .zip(fs::read(&new_plugin_settings).ok())
            .map(|(a, b)| a == b)
            .unwrap_or(false);
        if same {
            // caches/state regenerate under the new identity; drop the whole dir
            match fs::remove_dir_all(&old_plugin_dir) {
                Ok(_) => eprintln!("migration: removed {}", old_plugin_dir.display()),
                Err(e) => eprintln!(
                    "migration: could not remove {}: {}",
                    old_plugin_dir.display(),
                    e
                ),
            }
        } else {
            // Only nag when the new side is missing/broken (actionable). A valid
            // but different new file means the user customized it — the old dir
            // stays quietly as a backup.
            let new_valid = fs::read(&new_plugin_settings)
                .ok()
                .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
                .map(|v| v.is_object())
                .unwrap_or(false);
            if !new_valid {
                eprintln!(
                    "migration: keeping {} (settings not mirrored yet)",
                    old_plugin_dir.display()
                );
            }
        }
    }
}

/// Byte-compare every file under `old` against its counterpart under `new`.
/// Extra files on the new side are ignored. Symlinks/special files count as
/// a mismatch — when in doubt, keep the originals.
fn trees_identical(old: &Path, new: &Path) -> bool {
    for entry in walkdir::WalkDir::new(old).into_iter().filter_map(|e| e.ok()) {
        let rel = match entry.path().strip_prefix(old) {
            Ok(r) => r,
            Err(_) => return false,
        };
        if rel.as_os_str().is_empty() {
            continue; // the root itself
        }
        let counterpart = new.join(rel);
        if entry.file_type().is_dir() {
            if !counterpart.is_dir() {
                return false;
            }
        } else if entry.file_type().is_file() {
            let same = fs::read(entry.path())
                .ok()
                .zip(fs::read(&counterpart).ok())
                .map(|(a, b)| a == b)
                .unwrap_or(false);
            if !same {
                return false;
            }
        } else {
            return false;
        }
    }
    true
}

/// Copy entries missing on the destination side. Returns true if anything
/// was copied. Never overwrites or deletes.
fn merge_missing_entries(old: &Path, new: &Path) -> bool {
    let mut copied = false;
    if fs::create_dir_all(new).is_err() {
        return false;
    }
    let entries = match fs::read_dir(old) {
        Ok(e) => e,
        Err(_) => return false,
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let dst = new.join(entry.file_name());
        if dst.exists() {
            continue;
        }
        let res = if entry.path().is_dir() {
            copy_dir_all(&entry.path(), &dst)
        } else {
            fs::copy(entry.path(), &dst).map(|_| ())
        };
        if res.is_ok() {
            copied = true;
        } else {
            eprintln!(
                "migration: failed to copy {} -> {}",
                entry.path().display(),
                dst.display()
            );
        }
    }
    copied
}

/// Restore profile JSONs the app regenerated empty (empty mod_order while the
/// old copy has entries). The bad copy is kept next to it as *.bak.
fn repair_emptied_profiles(old_dir: &Path, new_dir: &Path) {
    let entries = match fs::read_dir(old_dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let new_file = new_dir.join(entry.file_name());
        if !new_file.is_file() {
            continue;
        }
        let old_text = fs::read_to_string(&p).unwrap_or_default();
        let new_text = fs::read_to_string(&new_file).unwrap_or_default();
        let old_count = serde_json::from_str::<serde_json::Value>(&old_text)
            .ok()
            .and_then(|v| v.get("mod_order")?.as_array().map(|a| a.len()))
            .unwrap_or(0);
        let new_count = serde_json::from_str::<serde_json::Value>(&new_text)
            .ok()
            .and_then(|v| v.get("mod_order")?.as_array().map(|a| a.len()))
            .unwrap_or(0);
        if old_count > 0 && new_count == 0 {
            let bak = new_file.with_extension("json.bak");
            let _ = fs::copy(&new_file, &bak);
            if fs::copy(&p, &new_file).is_ok() {
                eprintln!("migration: repaired profile {}", new_file.display());
            }
        }
    }
}

// --- Models ---
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mod {
    pub id: String,
    pub display_name: String,
    pub r#type: String, // "pak" | "folder"
    pub source_path: String,
    pub size_bytes: u64,
    pub has_lua: bool,
    pub has_pak: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub mod_order: Vec<String>,
    pub enabled: HashMap<String, bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub game_path: Option<String>,
    pub deploy_mode: String, // auto | symlink | copy
    pub active_profile: String,
    pub global_disable: bool,
    pub theme: String,
    pub font: String,
    #[serde(default)]
    pub auto_deploy: bool,
}
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            game_path: None,
            deploy_mode: "auto".into(),
            active_profile: "default".into(),
            global_disable: false,
            theme: "default".into(),
            font: "inter".into(),
            auto_deploy: false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImportResult {
    pub imported: Vec<String>,
    pub skipped: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct DeployResult {
    pub deployed: u32,
    pub errors: Vec<String>,
}

// --- Helpers ---
fn mod_size(p: &Path) -> u64 {
    if p.is_file() {
        return fs::metadata(p).map(|m| m.len()).unwrap_or(0);
    }
    let mut total = 0u64;
    for entry in walkdir::WalkDir::new(p).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            total += entry.metadata().map(|m| m.len()).unwrap_or(0);
        }
    }
    total
}
fn has_lua(p: &Path) -> bool {
    if p.is_file() {
        return p.extension().map(|e| e.eq_ignore_ascii_case("lua")).unwrap_or(false);
    }
    walkdir::WalkDir::new(p)
        .into_iter()
        .filter_map(|e| e.ok())
        .any(|e| e.path().extension().map(|ex| ex.eq_ignore_ascii_case("lua")).unwrap_or(false))
}
fn has_pak(p: &Path) -> bool {
    if p.is_file() {
        return p.extension().map(|e| e.eq_ignore_ascii_case("pak")).unwrap_or(false);
    }
    walkdir::WalkDir::new(p)
        .into_iter()
        .filter_map(|e| e.ok())
        .any(|e| e.path().extension().map(|ex| ex.eq_ignore_ascii_case("pak")).unwrap_or(false))
}

fn is_symlink(p: &Path) -> bool {
    fs::symlink_metadata(p).map(|m| m.file_type().is_symlink()).unwrap_or(false)
}

fn remove_deployed(p: &Path) -> Result<(), String> {
    if !p.exists() && !is_symlink(p) {
        return Ok(());
    }
    if is_symlink(p) {
        fs::remove_file(p).map_err(|e| format!("remove symlink {}: {}", p.display(), e))?;
        return Ok(());
    }
    if p.is_dir() {
        fs::remove_dir_all(p).map_err(|e| format!("rm dir {}: {}", p.display(), e))?;
    } else {
        fs::remove_file(p).map_err(|e| format!("rm file {}: {}", p.display(), e))?;
    }
    Ok(())
}

fn deploy_entry(source: &Path, dest: &Path, mode: &str) -> Result<String, String> {
    if dest.exists() || is_symlink(dest) {
        remove_deployed(dest)?;
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let chosen = if mode == "auto" { "symlink" } else { mode };
    if chosen == "symlink" || chosen == "junction" {
        let res: Result<(), std::io::Error> = {
            #[cfg(unix)]
            {
                std::os::unix::fs::symlink(source, dest)
            }
            #[cfg(windows)]
            {
                if source.is_dir() {
                    std::os::windows::fs::symlink_dir(source, dest)
                } else {
                    std::os::windows::fs::symlink_file(source, dest)
                }
            }
        };
        if res.is_ok() {
            return Ok("symlink".into());
        }
        // fallback to copy on privilege error or any error
    }
    // copy fallback
    if source.is_dir() {
        copy_dir_all(source, dest).map_err(|e| e.to_string())?;
    } else {
        fs::copy(source, dest).map_err(|e| e.to_string())?;
    }
    Ok("copy".into())
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst_path)?;
        } else {
            fs::copy(entry.path(), dst_path)?;
        }
    }
    Ok(())
}

fn safe_id(name: &str) -> String {
    let re = Regex::new(r"[^A-Za-z0-9._ \-]+").unwrap();
    let s = re.replace_all(name, "").trim().to_string();
    let s = s.replace(' ', "_");
    if s.is_empty() {
        "unnamed_mod".to_string()
    } else {
        s
    }
}

#[allow(dead_code)]
fn unique_store_path(base: PathBuf) -> PathBuf {
    if !base.exists() {
        return base;
    }
    let parent = base.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."));
    let stem = base
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "mod".to_string());
    let ext = base.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    for i in 1..100 {
        let cand = parent.join(format!("{}_{}{}", stem, i, ext));
        if !cand.exists() {
            return cand;
        }
    }
    base
}

fn extract_zip_to_temp(zip_path: &Path, temp_dir: &Path) -> Result<(), String> {
    let file = fs::File::open(zip_path).map_err(|e| format!("open zip {}: {}", zip_path.display(), e))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("read zip {}: {}", zip_path.display(), e))?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| format!("zip entry {}: {}", i, e))?;
        let name = entry.name().to_string();
        // skip dangerous paths and __MACOSX
        if name.contains("..") || name.starts_with('/') || name.starts_with('\\') {
            continue;
        }
        if name.starts_with("__MACOSX/") {
            continue;
        }
        let out_path = temp_dir.join(&name);
        if name.ends_with('/') {
            fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut outfile = fs::File::create(&out_path).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut outfile).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn can_symlink_probe() -> bool {
    let tmp = std::env::temp_dir();
    let src = tmp.join(format!("nms_probe_src_{}", uuid::Uuid::new_v4()));
    let link = tmp.join(format!("nms_probe_link_{}", uuid::Uuid::new_v4()));
    let _ = fs::create_dir_all(&src);
    let res = {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&src, &link).is_ok()
        }
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_dir(&src, &link).is_ok()
        }
    };
    let _ = fs::remove_dir_all(&src);
    let _ = fs::remove_file(&link);
    let _ = fs::remove_dir_all(&link);
    res
}

// --- Steam / game location ---
fn candidate_libraryfolders() -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/home/gloves"));
    let mut candidates = vec![
        home.join(".steam/steam/steamapps/libraryfolders.vdf"),
        home.join(".local/share/Steam/steamapps/libraryfolders.vdf"),
        home.join(".var/app/com.valvesoftware.Steam/data/Steam/steamapps/libraryfolders.vdf"),
        home.join(".steam/steamapps/libraryfolders.vdf"),
        home.join("snap/steam/common/.local/share/Steam/steamapps/libraryfolders.vdf"),
    ];
    // deduplicate
    let mut seen = std::collections::HashSet::new();
    let mut uniq = Vec::new();
    for c in candidates.drain(..) {
        let s = c.to_string_lossy().to_string();
        if seen.insert(s) {
            uniq.push(c);
        }
    }
    uniq
}
fn parse_library_paths(vdf_path: &Path) -> Vec<PathBuf> {
    let text = match fs::read_to_string(vdf_path) {
        Ok(t) => t,
        Err(_) => return vec![],
    };
    let re = Regex::new(r#""path"\s+"([^"]+)""#).unwrap();
    re.captures_iter(&text)
        .filter_map(|cap| cap.get(1).map(|m| PathBuf::from(m.as_str())))
        .collect()
}
fn has_nms(path: &Path) -> bool {
    let nms = path.join("steamapps/common/No Man's Sky");
    if nms.exists() && nms.join("GAMEDATA/PCBANKS/NMSARC.globals.pak").exists() {
        return true;
    }
    if path.join("GAMEDATA/PCBANKS/NMSARC.globals.pak").exists() {
        return true;
    }
    false
}
fn find_nms_install_inner(manual: Option<String>) -> Option<PathBuf> {
    if let Some(m) = manual {
        let p = PathBuf::from(shellexpand(&m));
        if p.exists() {
            for cand in [
                p.clone(),
                p.parent().map(|x| x.to_path_buf()).unwrap_or_else(|| p.clone()),
                p.parent()
                    .and_then(|x| x.parent())
                    .map(|x| x.to_path_buf())
                    .unwrap_or_else(|| p.clone()),
                p.parent()
                    .and_then(|x| x.parent())
                    .and_then(|x| x.parent())
                    .map(|x| x.to_path_buf())
                    .unwrap_or_else(|| p.clone()),
            ] {
                if cand.join("GAMEDATA/PCBANKS/NMSARC.globals.pak").exists() ||
                    (cand.file_name().map(|n| n == "No Man's Sky").unwrap_or(false) && cand.exists() && cand.join("GAMEDATA").exists())
                {
                    return Some(cand);
                }
            }
            if p.is_dir() && p.parent().map(|pr| pr.join("PCBANKS").exists()).unwrap_or(false) {
                if let Some(pr) = p.parent().and_then(|x| x.parent()) {
                    return Some(pr.to_path_buf());
                }
            }
            // fallback return p if exists but not validated? caller will check GAMEDATA
            return Some(p);
        }
    }
    if let Ok(env) = std::env::var("NMS_GAME_DIR") {
        if let Some(found) = find_nms_install_inner(Some(env.clone())) {
            if found.join("GAMEDATA").exists() {
                return Some(found);
            }
        }
    }
    if let Ok(env) = std::env::var("NMS_MODS_DIR") {
        if let Some(found) = find_nms_install_inner(Some(env.clone())) {
            if found.join("GAMEDATA").exists() {
                return Some(found);
            }
        }
    }
    for vdf in candidate_libraryfolders() {
        if !vdf.exists() {
            continue;
        }
        for lib in parse_library_paths(&vdf) {
            if has_nms(&lib) {
                let nms = lib.join("steamapps/common/No Man's Sky");
                if nms.exists() {
                    return Some(nms);
                }
                return Some(lib);
            }
            if lib.join("GAMEDATA/PCBANKS/NMSARC.globals.pak").exists() {
                if lib.file_name().map(|n| n == "No Man's Sky").unwrap_or(false) {
                    return Some(lib);
                }
                return Some(lib.parent().unwrap_or(&lib).to_path_buf());
            }
        }
    }
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let mut fallbacks = vec![
        home.join(".local/share/Steam/steamapps/common/No Man's Sky"),
        home.join(".steam/steam/steamapps/common/No Man's Sky"),
        home.join(".var/app/com.valvesoftware.Steam/data/Steam/steamapps/common/No Man's Sky"),
    ];
    for base in [PathBuf::from("/mnt"), PathBuf::from("/run/media")] {
        if base.exists() {
            if let Ok(children) = fs::read_dir(&base) {
                for child in children.filter_map(|c| c.ok()) {
                    let cand = child.path().join("SteamLibrary/steamapps/common/No Man's Sky");
                    if cand.exists() {
                        fallbacks.push(cand);
                    }
                }
            }
        }
    }
    #[cfg(windows)]
    {
        if let Some(cand) = ["C:", "D:", "E:", "F:"].iter().find_map(|drive| {
            [
                PathBuf::from(format!("{}/Program Files (x86)/Steam/steamapps/common/No Man's Sky", drive)),
                PathBuf::from(format!("{}/Steam/steamapps/common/No Man's Sky", drive)),
                PathBuf::from(format!("{}/SteamLibrary/steamapps/common/No Man's Sky", drive)),
            ].into_iter().find(|p| p.exists())
        }) {
            return Some(cand);
        }
    }
    fallbacks.into_iter().find(|f| f.exists() && f.join("GAMEDATA/PCBANKS/NMSARC.globals.pak").exists())
}
fn shellexpand(s: &str) -> String {
    if s.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return s.replacen("~/", &format!("{}/", home.display()), 1);
        }
    }
    s.to_string()
}

// --- Tauri Commands ---

#[tauri::command]
fn find_nms_install(manual: Option<String>) -> Option<String> {
    find_nms_install_inner(manual).map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
fn get_mods_dir(game_root: String) -> String {
    PathBuf::from(game_root).join("GAMEDATA/MODS").to_string_lossy().to_string()
}

#[tauri::command]
fn get_store_dir() -> String {
    store_dir().to_string_lossy().to_string()
}

#[tauri::command]
fn scan_store() -> Vec<Mod> {
    ensure_dirs();
    let store = store_dir();
    if !store.exists() {
        return vec![];
    }
    let mut mods = Vec::new();
    if let Ok(entries) = fs::read_dir(&store) {
        let mut children: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|x| x.path())).collect();
        children.sort_by_key(|a| a.file_name().unwrap_or_default().to_string_lossy().to_lowercase());
        for child in children {
            if child.file_name().map(|n| n.to_string_lossy().starts_with('.')).unwrap_or(false) {
                continue;
            }
            if child.is_file() {
                if child.extension().map(|e| e.eq_ignore_ascii_case("pak")).unwrap_or(false) {
                    let stem = child.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                    mods.push(Mod {
                        id: stem.clone(),
                        display_name: child.file_name().unwrap().to_string_lossy().to_string(),
                        r#type: "pak".into(),
                        source_path: child.to_string_lossy().to_string(),
                        size_bytes: mod_size(&child),
                        has_lua: false,
                        has_pak: true,
                    });
                }
                continue;
            }
            if child.is_dir() {
                let name = child.file_name().unwrap().to_string_lossy().to_string();
                mods.push(Mod {
                    id: name.clone(),
                    display_name: name,
                    r#type: "folder".into(),
                    source_path: child.to_string_lossy().to_string(),
                    size_bytes: mod_size(&child),
                    has_lua: has_lua(&child),
                    has_pak: has_pak(&child),
                });
            }
        }
    }
    mods
}

#[tauri::command]
fn import_mods(mods_dir: String, do_move: Option<bool>) -> Result<ImportResult, String> {
    ensure_dirs();
    let store = store_dir();
    fs::create_dir_all(&store).map_err(|e| e.to_string())?;
    let mods_path = PathBuf::from(shellexpand(&mods_dir));
    if !mods_path.exists() {
        return Ok(ImportResult { imported: vec![], skipped: vec![format!("Mods dir not found: {}", mods_path.display())] });
    }
    let do_move = do_move.unwrap_or(false);
    let re = Regex::new(r"^\d+[_-](.+)$").unwrap();
    let mut imported = Vec::new();
    let mut skipped = Vec::new();
    let entries = fs::read_dir(&mods_path).map_err(|e| e.to_string())?;
    for entry in entries.filter_map(|e| e.ok()) {
        let child = entry.path();
        let name = child.file_name().unwrap_or_default().to_string_lossy().to_string();
        if name == "DISABLEMODS.txt" || name.starts_with('.') {
            continue;
        }
        if is_symlink(&child) {
            skipped.push(format!("{}: already managed (symlink)", name));
            continue;
        }
        let store_name = if let Some(cap) = re.captures(&name) {
            cap.get(1).unwrap().as_str().to_string()
        } else {
            name.clone()
        };
        let dest = store.join(&store_name);
        if dest.exists() {
            skipped.push(format!("{}: store already has {} (collision)", name, store_name));
            continue;
        }
        let res: Result<(), String> = if child.is_file() {
            if do_move {
                fs::rename(&child, &dest).map_err(|e| e.to_string())
            } else {
                fs::copy(&child, &dest).map(|_| ()).map_err(|e| e.to_string())
            }
        } else if child.is_dir() {
            if do_move {
                fs::rename(&child, &dest).map_err(|e| e.to_string())
            } else {
                copy_dir_all(&child, &dest).map_err(|e| e.to_string())
            }
        } else {
            Err("unknown type".into())
        };
        match res {
            Ok(_) => imported.push(store_name),
            Err(e) => skipped.push(format!("{}: failed {}", name, e)),
        }
    }
    Ok(ImportResult { imported, skipped })
}

#[tauri::command]
fn add_mods(paths: Vec<String>) -> Result<ImportResult, String> {
    ensure_dirs();
    let store = store_dir();
    fs::create_dir_all(&store).map_err(|e| e.to_string())?;
    let mut imported = Vec::new();
    let mut skipped = Vec::new();
    for raw in paths {
        let p = PathBuf::from(shellexpand(&raw));
        if !p.exists() {
            skipped.push(format!("{}: not found", raw));
            continue;
        }
        let fname = p.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_else(|| "unnamed".into());
        if p.is_file() {
            let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            if ext == "zip" {
                let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "unnamed_mod".into());
                let base_id = safe_id(&stem);
                let dest = store.join(&base_id);
                if dest.exists() || store.join(format!("{}.pak", base_id)).exists() {
                    skipped.push(format!("{}: already exists in store (skipped duplicate)", fname));
                    continue;
                }
                let final_id = dest.file_name().unwrap().to_string_lossy().to_string();
                // extract to temp
                let tmp = std::env::temp_dir().join(format!("nms_add_{}_{}", final_id, uuid::Uuid::new_v4()));
                fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
                let extract_res = extract_zip_to_temp(&p, &tmp);
                if let Err(e) = extract_res {
                    skipped.push(format!("{}: zip extract failed {}", fname, e));
                    let _ = fs::remove_dir_all(&tmp);
                    continue;
                }
                // inspect temp contents
                let entries: Vec<PathBuf> = fs::read_dir(&tmp).map(|r| r.filter_map(|e| e.ok().map(|x| x.path())).collect()).unwrap_or_default();
                // filter __MACOSX already skipped, but check
                let entries: Vec<PathBuf> = entries.into_iter().filter(|x| x.file_name().map(|n| n != "__MACOSX").unwrap_or(true)).collect();
                if entries.is_empty() {
                    skipped.push(format!("{}: zip empty", fname));
                    let _ = fs::remove_dir_all(&tmp);
                    continue;
                }
                // Auto-detect: single file .pak -> treat as pak mod
                if entries.len() == 1 && entries[0].is_file() && entries[0].extension().map(|e| e.eq_ignore_ascii_case("pak")).unwrap_or(false) {
                    let pak_src = &entries[0];
                    let safe_pak_stem = safe_id(&pak_src.file_stem().unwrap().to_string_lossy());
                    let pak_dest = store.join(format!("{}.pak", safe_pak_stem));
                    if pak_dest.exists() {
                        skipped.push(format!("{}: already exists in store (skipped duplicate)", fname));
                        let _ = fs::remove_dir_all(&tmp);
                        continue;
                    }
                    match fs::copy(pak_src, &pak_dest) {
                        Ok(_) => {
                            let id = pak_dest.file_stem().unwrap().to_string_lossy().to_string();
                            imported.push(id);
                        },
                        Err(e) => skipped.push(format!("{}: copy pak failed {}", fname, e)),
                    }
                    let _ = fs::remove_dir_all(&tmp);
                    continue;
                }
                // Auto-detect: single top folder -> use that folder contents
                let final_dest: PathBuf;
                if entries.len() == 1 && entries[0].is_dir() {
                    // single folder -> move that folder to dest (rename to safe_id already)
                    final_dest = dest;
                    // The single entry may have original name different from safe_id; we move the folder content via rename
                    let src_single = &entries[0];
                    match fs::rename(src_single, &final_dest) {
                        Ok(_) => {
                            let _ = fs::remove_dir_all(&tmp);
                            imported.push(final_id);
                        },
                        Err(_) => {
                            // fallback copy
                            match copy_dir_all(src_single, &final_dest) {
                                Ok(_) => {
                                    let _ = fs::remove_dir_all(&tmp);
                                    imported.push(final_id);
                                },
                                Err(e) => {
                                    skipped.push(format!("{}: copy folder failed {}", fname, e));
                                    let _ = fs::remove_dir_all(&tmp);
                                }
                            }
                        }
                    }
                } else {
                    // multiple entries or single non-pak file -> create dest folder and move all entries in
                    final_dest = dest;
                    fs::create_dir_all(&final_dest).map_err(|e| e.to_string())?;
                    let mut ok = true;
                    for ent in &entries {
                        let dst = final_dest.join(ent.file_name().unwrap());
                        let res = if ent.is_dir() { copy_dir_all(ent, &dst) } else { fs::copy(ent, &dst).map(|_| ()) };
                        if res.is_err() { ok = false; break; }
                    }
                    let _ = fs::remove_dir_all(&tmp);
                    if ok {
                        imported.push(final_id);
                    } else {
                        skipped.push(format!("{}: extract copy failed", fname));
                        let _ = fs::remove_dir_all(&final_dest);
                    }
                }
            } else if ext == "pak" {
                let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "unnamed".into());
                let base_id = safe_id(&stem);
                let dest = store.join(format!("{}.pak", base_id));
                if dest.exists() {
                    skipped.push(format!("{}: already exists in store (skipped duplicate)", fname));
                    continue;
                }
                match fs::copy(&p, &dest) {
                    Ok(_) => {
                        let id = dest.file_stem().unwrap().to_string_lossy().to_string();
                        imported.push(id);
                    },
                    Err(e) => skipped.push(format!("{}: copy failed {}", fname, e)),
                }
            } else {
                skipped.push(format!("{}: unsupported file type .{} (only .pak/.zip supported)", fname, ext));
            }
        } else if p.is_dir() {
            let raw_name = p.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_else(|| "unnamed_mod".into());
            let base_id = safe_id(&raw_name);
            let dest = store.join(&base_id);
            if dest.exists() {
                skipped.push(format!("{}: already exists in store (skipped duplicate)", fname));
                continue;
            }
            let final_id = dest.file_name().unwrap().to_string_lossy().to_string();
            match copy_dir_all(&p, &dest) {
                Ok(_) => imported.push(final_id),
                Err(e) => skipped.push(format!("{}: copy folder failed {}", fname, e)),
            }
        } else {
            skipped.push(format!("{}: unknown type", fname));
        }
    }
    Ok(ImportResult { imported, skipped })
}

fn load_profile_inner(name: &str) -> Profile {
    let p = profiles_dir().join(format!("{}.json", name));
    if p.exists() {
        if let Ok(text) = fs::read_to_string(&p) {
            if let Ok(mut prof) = serde_json::from_str::<Profile>(&text) {
                // sync with store
                let store_mods = scan_store();
                let mut store_ids: std::collections::HashSet<String> = store_mods.iter().map(|m| m.id.clone()).collect();
                let mut changed = false;
                for mid in store_ids.drain() {
                    if !prof.mod_order.contains(&mid) {
                        prof.mod_order.push(mid.clone());
                        changed = true;
                    }
                    if !prof.enabled.contains_key(&mid) {
                        prof.enabled.insert(mid.clone(), true);
                        changed = true;
                    }
                }
                if changed {
                    let _ = save_profile_inner(&prof);
                }
                return prof;
            }
        }
    }
    // create
    let store_mods = scan_store();
    let mut order: Vec<String> = store_mods.iter().map(|m| m.id.clone()).collect();
    order.sort_by_key(|a| a.to_lowercase());
    let enabled = store_mods.iter().map(|m| (m.id.clone(), true)).collect();
    let prof = Profile { name: name.to_string(), mod_order: order, enabled };
    let _ = save_profile_inner(&prof);
    prof
}
fn save_profile_inner(prof: &Profile) -> Result<(), String> {
    ensure_dirs();
    let p = profiles_dir().join(format!("{}.json", prof.name));
    let tmp = p.with_extension("tmp");
    let text = serde_json::to_string_pretty(prof).map_err(|e| e.to_string())?;
    fs::write(&tmp, text).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &p).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn scan_deployed(mods_dir: String) -> HashMap<String, String> {
    let path = PathBuf::from(shellexpand(&mods_dir));
    let mut map = HashMap::new();
    if let Ok(entries) = fs::read_dir(&path) {
        for entry in entries.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_string();
            if name == "DISABLEMODS.txt" || name.starts_with('.') { continue; }
            map.insert(name.clone(), entry.path().to_string_lossy().to_string());
        }
    }
    map
}

#[tauri::command]
fn deploy_mods(mods_dir: String, ordered_ids: Vec<String>, deploy_mode: Option<String>) -> Result<DeployResult, String> {
    let mods_path = PathBuf::from(shellexpand(&mods_dir));
    fs::create_dir_all(&mods_path).map_err(|e| e.to_string())?;
    let mode = deploy_mode.unwrap_or_else(|| "auto".into());
    let re = Regex::new(r"^\d{2,3}[_-]").unwrap();
    let mut errors = Vec::new();
    // clear
    if let Ok(entries) = fs::read_dir(&mods_path) {
        for entry in entries.filter_map(|e| e.ok()) {
            let p = entry.path();
            let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
            if name == "DISABLEMODS.txt" { continue; }
            let is_link = is_symlink(&p);
            let has_prefix = re.is_match(&name);
            if is_link || has_prefix {
                if let Err(e) = remove_deployed(&p) {
                    errors.push(format!("cleanup {}: {}", name, e));
                }
            }
        }
    }
    // build map
    let store_mods: HashMap<String, PathBuf> = scan_store().into_iter().map(|m| (m.id.clone(), PathBuf::from(m.source_path))).collect();
    let store_mods_type: HashMap<String, String> = scan_store().into_iter().map(|m| (m.id.clone(), m.r#type.clone())).collect();
    // Need actual Mod objects for deploy decisions (pak vs folder)
    // For pak, need file name
    let mut deployed = 0u32;
    for (idx, mid) in ordered_ids.iter().enumerate() {
        let src = match store_mods.get(mid) {
            Some(p) => p.clone(),
            None => { errors.push(format!("{}: not found in store", mid)); continue; }
        };
        let typ = store_mods_type.get(mid).cloned().unwrap_or_else(|| "folder".into());
        let prefix = if ordered_ids.len() < 100 { format!("{:02}_", idx+1) } else { format!("{:03}_", idx+1) };
        let deployed_name = if typ == "pak" {
            let fname = src.file_name().unwrap_or_default().to_string_lossy().to_string();
            format!("{}{}", prefix, fname)
        } else {
            format!("{}{}", prefix, mid)
        };
        let dest = mods_path.join(deployed_name);
        match deploy_entry(&src, &dest, &mode) {
            Ok(_) => deployed += 1,
            Err(e) => errors.push(format!("{}: {}", mid, e)),
        }
    }
    Ok(DeployResult { deployed, errors })
}

#[tauri::command]
fn global_disable_enabled(mods_dir: String) -> bool {
    let p = PathBuf::from(shellexpand(&mods_dir)).join("DISABLEMODS.txt");
    p.exists()
}

#[tauri::command]
fn set_global_disable(mods_dir: String, disable: bool) -> Result<(), String> {
    let mods_path = PathBuf::from(shellexpand(&mods_dir));
    fs::create_dir_all(&mods_path).map_err(|e| e.to_string())?;
    let flag = mods_path.join("DISABLEMODS.txt");
    let pc_flag = mods_path.parent().map(|p| p.join("PCBANKS/DISABLEMODS.txt")).unwrap_or_else(|| PathBuf::from(""));
    if disable {
        fs::write(&flag, "").map_err(|e| e.to_string())?;
    } else {
        let _ = fs::remove_file(&flag);
        let _ = fs::remove_file(&pc_flag);
    }
    Ok(())
}

#[tauri::command]
fn list_profiles() -> Vec<String> {
    ensure_dirs();
    let dir = profiles_dir();
    if !dir.exists() { return vec![]; }
    let mut names = vec![];
    if let Ok(entries) = fs::read_dir(&dir) {
        for e in entries.filter_map(|e| e.ok()) {
            if e.path().extension().map(|x| x == "json").unwrap_or(false) {
                if let Some(stem) = e.path().file_stem().map(|s| s.to_string_lossy().to_string()) {
                    names.push(stem);
                }
            }
        }
    }
    names.sort();
    names
}

#[tauri::command]
fn load_profile(name: String) -> Profile {
    load_profile_inner(&name)
}

#[tauri::command]
fn save_profile(profile: Profile) -> Result<(), String> {
    save_profile_inner(&profile)
}

#[tauri::command]
fn create_profile(name: String, clone_from: Option<String>) -> Result<Profile, String> {
    if profiles_dir().join(format!("{}.json", name)).exists() {
        return Err(format!("Profile {} already exists", name));
    }
    let prof = if let Some(src) = clone_from {
        let mut p = load_profile_inner(&src);
        p.name = name.clone();
        p
    } else {
        let store_mods = scan_store();
        let mut order: Vec<String> = store_mods.iter().map(|m| m.id.clone()).collect();
        order.sort_by_key(|a| a.to_lowercase());
        let enabled = store_mods.iter().map(|m| (m.id.clone(), true)).collect();
        Profile { name: name.clone(), mod_order: order, enabled }
    };
    save_profile_inner(&prof)?;
    Ok(prof)
}

#[tauri::command]
fn delete_profile(name: String) -> Result<(), String> {
    let p = profiles_dir().join(format!("{}.json", name));
    if p.exists() { fs::remove_file(p).map_err(|e| e.to_string())?; }
    if list_profiles().is_empty() {
        let _ = create_profile("default".into(), None);
    }
    Ok(())
}

#[tauri::command]
fn rename_profile(old: String, new: String) -> Result<(), String> {
    if old == new { return Ok(()); }
    let src = profiles_dir().join(format!("{}.json", old));
    let dst = profiles_dir().join(format!("{}.json", new));
    if !src.exists() { return Err("source not found".into()); }
    if dst.exists() { return Err(format!("Profile {} already exists", new)); }
    let mut prof = load_profile_inner(&old);
    prof.name = new.clone();
    save_profile_inner(&prof)?;
    fs::remove_file(src).map_err(|e| e.to_string())?;
    Ok(())
}

fn load_config_from_store(app: &tauri::AppHandle) -> AppConfig {
    use tauri_plugin_store::StoreExt;
    // Try Tauri store first (native)
    if let Ok(store) = app.store("settings.json") {
        let mut cfg = AppConfig::default();
        if let Some(v) = store.get("game_path") {
            cfg.game_path = serde_json::from_value(v.clone()).unwrap_or(None);
        }
        if let Some(v) = store.get("deploy_mode") {
            if let Ok(s) = serde_json::from_value::<String>(v.clone()) {
                cfg.deploy_mode = s;
            }
        }
        if let Some(v) = store.get("active_profile") {
            if let Ok(s) = serde_json::from_value::<String>(v.clone()) {
                cfg.active_profile = s;
            }
        }
        if let Some(v) = store.get("global_disable") {
            if let Ok(b) = serde_json::from_value::<bool>(v.clone()) {
                cfg.global_disable = b;
            }
        }
        if let Some(v) = store.get("auto_deploy") {
            if let Ok(b) = serde_json::from_value::<bool>(v.clone()) {
                cfg.auto_deploy = b;
            }
        }
        if let Some(v) = store.get("theme") {
            if let Ok(s) = serde_json::from_value::<String>(v.clone()) {
                cfg.theme = s;
            }
        }
        if let Some(v) = store.get("font") {
            if let Ok(s) = serde_json::from_value::<String>(v.clone()) {
                cfg.font = s;
            }
        }
        // If store has any keys, consider it migrated
        if store.get("game_path").is_some()
            || store.get("deploy_mode").is_some()
            || store.get("active_profile").is_some()
            || store.get("theme").is_some()
            || store.get("font").is_some()
            || store.get("auto_deploy").is_some()
        {
            return cfg;
        }
        // Fallback: migrate legacy file at nms-mod-manager/settings.json
        let legacy = config_file();
        if legacy.exists() {
            if let Ok(t) = fs::read_to_string(&legacy) {
                if let Ok(legacy_cfg) = serde_json::from_str::<AppConfig>(&t) {
                    // seed store
                    store.set("game_path", serde_json::to_value(&legacy_cfg.game_path).unwrap());
                    store.set("deploy_mode", serde_json::to_value(&legacy_cfg.deploy_mode).unwrap());
                    store.set("active_profile", serde_json::to_value(&legacy_cfg.active_profile).unwrap());
                    store.set("global_disable", serde_json::to_value(legacy_cfg.global_disable).unwrap());
                    store.set("auto_deploy", serde_json::to_value(legacy_cfg.auto_deploy).unwrap());
                    store.set("theme", serde_json::to_value(&legacy_cfg.theme).unwrap());
                    store.set("font", serde_json::to_value(&legacy_cfg.font).unwrap());
                    return legacy_cfg;
                }
            }
        }
        // check legacy has at least empty default
        return cfg;
    }
    // fallback to legacy file if store unavailable
    let p = config_file();
    if p.exists() {
        if let Ok(t) = fs::read_to_string(&p) {
            if let Ok(cfg) = serde_json::from_str::<AppConfig>(&t) {
                return cfg;
            }
        }
    }
    AppConfig::default()
}

fn save_config_to_store(app: &tauri::AppHandle, config: &AppConfig) -> Result<(), String> {
    use tauri_plugin_store::StoreExt;
    if let Ok(store) = app.store("settings.json") {
        store.set("game_path", serde_json::to_value(&config.game_path).map_err(|e| e.to_string())?);
        store.set("deploy_mode", serde_json::to_value(&config.deploy_mode).map_err(|e| e.to_string())?);
        store.set("active_profile", serde_json::to_value(&config.active_profile).map_err(|e| e.to_string())?);
        store.set("global_disable", serde_json::to_value(config.global_disable).map_err(|e| e.to_string())?);
        store.set("auto_deploy", serde_json::to_value(config.auto_deploy).map_err(|e| e.to_string())?);
        store.set("theme", serde_json::to_value(&config.theme).map_err(|e| e.to_string())?);
        store.set("font", serde_json::to_value(&config.font).map_err(|e| e.to_string())?);
        // also keep legacy file for Python compat
        ensure_dirs();
        let p = config_file();
        let tmp = p.with_extension("tmp");
        let text = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
        let _ = fs::write(&tmp, text);
        let _ = fs::rename(&tmp, &p);
        return Ok(());
    }
    // fallback legacy
    ensure_dirs();
    let p = config_file();
    let tmp = p.with_extension("tmp");
    let text = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(&tmp, text).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &p).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn load_config(app: tauri::AppHandle) -> AppConfig {
    load_config_from_store(&app)
}

#[tauri::command]
fn save_config(app: tauri::AppHandle, config: AppConfig) -> Result<(), String> {
    save_config_to_store(&app, &config)
}

#[tauri::command]
fn rename_store_mod(id: String, new_name: String) -> Result<String, String> {
    let mods = scan_store();
    let m = mods
        .iter()
        .find(|x| x.id == id)
        .ok_or_else(|| format!("Mod {} not found in store", id))?;
    let src = PathBuf::from(&m.source_path);
    let safe = safe_id(&new_name);
    let dest = if m.r#type == "pak" {
        store_dir().join(format!("{}.pak", safe))
    } else {
        store_dir().join(&safe)
    };
    let new_id = if m.r#type == "pak" {
        dest.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| safe.clone())
    } else {
        safe.clone()
    };
    if new_id == id {
        return Ok(id);
    }
    if dest.exists() {
        return Err("A mod with that name already exists".into());
    }
    fs::rename(&src, &dest).map_err(|e| e.to_string())?;
    Ok(new_id)
}

#[tauri::command]
fn remove_store_mod(id: String) -> Result<(), String> {
    // Find mod by id
    let mods = scan_store();
    if let Some(m) = mods.iter().find(|x| x.id == id) {
        let p = PathBuf::from(&m.source_path);
        remove_deployed(&p)?;
        return Ok(());
    }
    // fallback: try store_dir directly
    let p = store_dir().join(&id);
    if p.exists() {
        remove_deployed(&p)?;
        return Ok(());
    }
    let p2 = store_dir().join(format!("{}.pak", id));
    if p2.exists() {
        remove_deployed(&p2)?;
        return Ok(());
    }
    Err(format!("Mod {} not found in store", id))
}

#[tauri::command]
fn can_symlink() -> bool {
    can_symlink_probe()
}

#[tauri::command]
fn open_folder(path: String) -> Result<(), String> {
    tauri_plugin_opener::open_path(&path, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_downloads_dir() -> Result<String, String> {
    dirs::download_dir().map(|p| p.to_string_lossy().to_string()).ok_or_else(|| "no downloads dir".into())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    migrate_legacy_data_dir();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(Mutex::new(saves::SaveState::default()))
        .invoke_handler(tauri::generate_handler![
            find_nms_install,
            get_mods_dir,
            get_store_dir,
            scan_store,
            scan_deployed,
            import_mods,
            add_mods,
            deploy_mods,
            global_disable_enabled,
            set_global_disable,
            list_profiles,
            load_profile,
            save_profile,
            create_profile,
            delete_profile,
            rename_profile,
            load_config,
            save_config,
            can_symlink,
            remove_store_mod,
            rename_store_mod,
            open_folder,
            get_downloads_dir,
            saves::find_save_dirs,
            saves::find_save_dir,
            saves::list_save_files,
            saves::list_save_subdirs,
            saves::decompress_save,
            saves::list_bases,
            saves::get_base_json,
            saves::get_nmsbase_text,
            saves::read_text_file,
            saves::export_base,
            saves::export_nmsbase,
            saves::import_base,
            saves::recompress_save,
            saves::backup_saves,
            saves::list_backups,
            saves::restore_save
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod migration_tests {
    use super::*;

    struct EnvGuard {
        data_home: Option<std::ffi::OsString>,
        config_home: Option<std::ffi::OsString>,
    }
    impl EnvGuard {
        fn set_fake(base: &Path) -> Self {
            let g = Self {
                data_home: std::env::var_os("XDG_DATA_HOME"),
                config_home: std::env::var_os("XDG_CONFIG_HOME"),
            };
            std::env::set_var("XDG_DATA_HOME", base.join(".local/share"));
            std::env::set_var("XDG_CONFIG_HOME", base.join(".config"));
            g
        }
    }
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.data_home {
                Some(v) => std::env::set_var("XDG_DATA_HOME", v),
                None => std::env::remove_var("XDG_DATA_HOME"),
            }
            match &self.config_home {
                Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
                None => std::env::remove_var("XDG_CONFIG_HOME"),
            }
        }
    }

    const OLD_PROFILE: &str = r#"{"name":"default","mod_order":["ModA","ModB"],"enabled":{"ModA":true,"ModB":false}}"#;

    #[test]
    fn migration_copies_merges_repairs_and_cleans_up() {
        let _env = ENV_LOCK.lock().unwrap();
        let base = std::env::temp_dir().join(format!("nmm_migtest_{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let _guard = EnvGuard::set_fake(&base);

        let data = base.join(".local/share");
        let cfg = base.join(".config");
        let old_plugin_settings = r#"{"theme":"rose-pine-moon"}"#;

        // (re)seed the old world: 2 mods (one nested), full profile, settings
        let seed_old_data = || {
            let old_data = data.join("nms-mod-manager");
            fs::create_dir_all(old_data.join("mods/ModA")).unwrap();
            fs::write(old_data.join("mods/ModA/mod.pak"), b"pak-a").unwrap();
            fs::create_dir_all(old_data.join("mods/ModB")).unwrap();
            fs::write(old_data.join("mods/ModB/readme.txt"), b"hi").unwrap();
            fs::create_dir_all(old_data.join("profiles")).unwrap();
            fs::write(old_data.join("profiles/default.json"), OLD_PROFILE).unwrap();
        };
        let seed_old_plugin = || {
            fs::create_dir_all(data.join("dev.gloved.nomodssky")).unwrap();
            fs::write(
                data.join("dev.gloved.nomodssky/settings.json"),
                old_plugin_settings,
            )
            .unwrap();
        };

        // phase A: first run copies everything, then removes verified old dirs
        seed_old_data();
        seed_old_plugin();
        fs::create_dir_all(cfg.join("nms-mod-manager")).unwrap();
        fs::write(cfg.join("nms-mod-manager/settings.json"), b"{}").unwrap();
        migrate_legacy_data_dir();
        let new_data = data.join("nomansmanager");
        assert_eq!(fs::read(new_data.join("mods/ModA/mod.pak")).unwrap(), b"pak-a");
        assert_eq!(
            fs::read_to_string(new_data.join("profiles/default.json")).unwrap(),
            OLD_PROFILE
        );
        assert!(new_data.join("mods/ModB/readme.txt").is_file());
        assert!(cfg.join("nomansmanager/settings.json").is_file());
        assert_eq!(
            fs::read_to_string(data.join("dev.gloved.nomansmanager/settings.json")).unwrap(),
            old_plugin_settings
        );
        assert!(!data.join("nms-mod-manager").exists(), "mirrored old data dir removed");
        assert!(!cfg.join("nms-mod-manager").exists(), "mirrored old config dir removed");
        assert!(
            !data.join("dev.gloved.nomodssky").exists(),
            "mirrored old plugin dir removed"
        );

        // phase B: poisoned state (the reported bug) self-heals from re-seeded old dirs
        seed_old_data();
        let _ = fs::remove_dir_all(new_data.join("mods"));
        fs::create_dir_all(new_data.join("mods")).unwrap();
        fs::write(
            new_data.join("profiles/default.json"),
            r#"{"name":"default","mod_order":[],"enabled":{}}"#,
        )
        .unwrap();
        migrate_legacy_data_dir();
        assert_eq!(fs::read(new_data.join("mods/ModA/mod.pak")).unwrap(), b"pak-a");
        assert_eq!(
            fs::read_to_string(new_data.join("profiles/default.json")).unwrap(),
            OLD_PROFILE
        );
        assert!(new_data.join("profiles/default.json.bak").is_file());
        assert!(!data.join("nms-mod-manager").exists(), "re-mirrored old data dir removed");

        // phase C: diverged new settings are never clobbered; old dir kept...
        seed_old_plugin();
        fs::write(
            data.join("dev.gloved.nomansmanager/settings.json"),
            r#"{"theme":"custom"}"#,
        )
        .unwrap();
        migrate_legacy_data_dir();
        assert_eq!(
            fs::read_to_string(data.join("dev.gloved.nomansmanager/settings.json")).unwrap(),
            r#"{"theme":"custom"}"#
        );
        assert!(data.join("dev.gloved.nomodssky").exists(), "diverged old dir kept");
        // ...until they match, then cleanup proceeds
        fs::write(
            data.join("dev.gloved.nomansmanager/settings.json"),
            old_plugin_settings,
        )
        .unwrap();
        migrate_legacy_data_dir();
        assert!(!data.join("dev.gloved.nomodssky").exists(), "matched old dir removed");

        let _ = fs::remove_dir_all(&base);
    }
}

#[cfg(test)]
mod rename_tests {
    use super::*;

    #[test]
    fn rename_folder_pak_collision_and_sanitize() {
        let _env = ENV_LOCK.lock().unwrap();
        let base = std::env::temp_dir().join(format!("nmm_renametest_{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        std::env::set_var("XDG_DATA_HOME", base.join(".local/share"));
        let store = base.join(".local/share/nomansmanager/mods");

        fs::create_dir_all(store.join("Old_Folder")).unwrap();
        fs::write(store.join("Old_Folder/file.txt"), b"x").unwrap();
        fs::write(store.join("OldShip.pak"), b"pak").unwrap();
        fs::create_dir_all(store.join("Taken")).unwrap();
        fs::write(store.join("Other.pak"), b"other").unwrap();

        // folder rename, spaces become underscores via safe_id
        let id = rename_store_mod("Old_Folder".into(), "New Cool Mod".into()).unwrap();
        assert_eq!(id, "New_Cool_Mod");
        assert!(store.join("New_Cool_Mod/file.txt").is_file());
        assert!(!store.join("Old_Folder").exists());

        // pak keeps its extension
        let id2 = rename_store_mod("OldShip".into(), "NewShip".into()).unwrap();
        assert_eq!(id2, "NewShip");
        assert!(store.join("NewShip.pak").is_file());

        // collision rejected
        assert!(rename_store_mod("NewShip".into(), "Other".into()).is_err());
        assert!(store.join("NewShip.pak").is_file());

        // no-op rename
        assert_eq!(
            rename_store_mod("NewShip".into(), "NewShip".into()).unwrap(),
            "NewShip"
        );

        std::env::remove_var("XDG_DATA_HOME");
        let _ = fs::remove_dir_all(&base);
    }
}

#[cfg(test)]
mod replay_tests {
    use super::*;
    use crate::ENV_LOCK;

    #[test]
    fn replay_add_then_rename_betterflight() {
        let _env = ENV_LOCK.lock().unwrap();
        let base = std::env::temp_dir().join(format!("nmm_replay_{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        std::env::set_var("XDG_DATA_HOME", base.join(".local/share"));
        std::env::set_var("XDG_CONFIG_HOME", base.join(".config"));

        let zip = "/home/gloves/Downloads/BetterFlight 1.2.0 4475 5 2026-09-15T23-03Z n0CrIxzUc.zip";
        let res = add_mods(vec![zip.to_string()]).expect("add_mods");
        println!("imported={:?} skipped={:?}", res.imported, res.skipped);
        assert_eq!(res.imported.len(), 1, "zip should import");
        let id = res.imported[0].clone();

        let after_add: Vec<String> = scan_store().iter().map(|m| m.id.clone()).collect();
        println!("store after add: {:?}", after_add);
        assert!(after_add.contains(&id));

        let new_id = rename_store_mod(id.clone(), "better flight mod".into()).expect("rename");
        println!("renamed {} -> {}", id, new_id);
        let after_rename: Vec<String> = scan_store().iter().map(|m| m.id.clone()).collect();
        println!("store after rename: {:?}", after_rename);
        assert!(after_rename.contains(&new_id), "renamed mod must still be listed");
        assert!(!after_rename.contains(&id));

        std::env::remove_var("XDG_DATA_HOME");
        std::env::remove_var("XDG_CONFIG_HOME");
        let _ = fs::remove_dir_all(&base);
    }
}
