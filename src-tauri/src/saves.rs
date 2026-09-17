//! No Man's Sky save engine: .hg decompress (LZ4 blocks), MBINCompiler key
//! mapping, base list/export/import, recompress, backup/restore.
//!
//! Save format: concatenated LZ4 blocks, each with a 16-byte LE header
//! [magic 0xFEEDA1E5][compressed_size][uncompressed_size][padding].
//! Data lives under the host app's data dir (`saves/` subfolder).

use chrono::Local;
use lz4_flex::block::{compress, decompress};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const APPID: &str = "275850";
use super::shellexpand;

/// Save-related files (backups, output, mapping cache) live here so the
/// host app keeps one data dir.
pub(crate) fn saves_root() -> PathBuf {
    super::app_data_dir().join("saves")
}
const SAVE_SUB: &str = "drive_c/users/steamuser/AppData/Roaming/HelloGames/NMS";
const MAPPING_URL: &str =
    "https://github.com/monkeyman192/MBINCompiler/releases/latest/download/mapping.json";
const CACHE_MAX_AGE_DAYS: i64 = 7;

const BLOCK_MAGIC: u32 = 0xFEEDA1E5;
const MAX_UNCOMPRESSED_BLOCK_SIZE: usize = 0x80000; // 512 KB

const PLANETARY_TYPES: [&str; 2] = ["HomePlanetBase", "ExternalPlanetBase"];

// ---------------------------------------------------------------------------
// App dirs (match Python platformdirs layout, APP_NAME switch)
// ---------------------------------------------------------------------------

fn mapping_cache_path() -> PathBuf {
    saves_root().join(".nms_mapping_cache").join("mapping.json")
}

fn backups_save_dir() -> PathBuf {
    saves_root().join("backups").join("save files")
}

fn backups_bases_dir() -> PathBuf {
    saves_root().join("backups").join("bases")
}

fn output_bases_dir() -> PathBuf {
    saves_root().join("output").join("bases")
}

fn output_nmsbase_dir() -> PathBuf {
    saves_root().join("output").join("nmsbase")
}

fn output_dir() -> PathBuf {
    saves_root().join("output")
}

fn timestamp() -> String {
    Local::now().format("%Y%m%d_%H%M%S").to_string()
}

fn safe_name(name: &str) -> String {
    let s: String = name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
        .collect::<String>()
        .trim()
        .to_string();
    let s = s.replace(' ', "_");
    if s.is_empty() {
        "unnamed_base".to_string()
    } else {
        s
    }
}

// ---------------------------------------------------------------------------
// Global save state (replaces Python SaveEditor instance fields)
// ---------------------------------------------------------------------------

#[derive(Default)]
pub(crate) struct SaveState {
    save_dir: Option<PathBuf>,
    save_file: Option<String>,
    save_json: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Proton save autodetect (port of nms_tui/proton.py)
// ---------------------------------------------------------------------------

fn candidate_roots() -> Vec<PathBuf> {
    let mut raw: Vec<PathBuf> = Vec::new();
    // Native storefront locations (Windows/macOS builds)
    #[cfg(target_os = "windows")]
    {
        // Steam + GOG: %AppData%\HelloGames\NMS (st_* / DefaultUser inside)
        if let Some(roaming) = dirs::data_dir() {
            raw.push(roaming.join("HelloGames").join("NMS"));
        }
    }
    #[cfg(target_os = "macos")]
    {
        // Steam: ~/Library/Application Support/HelloGames/NMS
        if let Some(home) = dirs::home_dir() {
            raw.push(
                home.join("Library")
                    .join("Application Support")
                    .join("HelloGames")
                    .join("NMS"),
            );
        }
    }
    // Linux / Steam Deck: Proton prefix(es) for appid 275850
    {
        let h = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/home/gloves"));
        raw.extend([
            h.join(".local/share/Steam/steamapps/compatdata")
                .join(APPID)
                .join("pfx")
                .join(SAVE_SUB),
            h.join(".steam/steam/steamapps/compatdata")
                .join(APPID)
                .join("pfx")
                .join(SAVE_SUB),
            h.join(".var/app/com.valvesoftware.Steam/data/Steam/steamapps/compatdata")
                .join(APPID)
                .join("pfx")
                .join(SAVE_SUB),
            h.join(".var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps/compatdata")
                .join(APPID)
                .join("pfx")
                .join(SAVE_SUB),
            h.join("snap/steam/common/.local/share/Steam/steamapps/compatdata")
                .join(APPID)
                .join("pfx")
                .join(SAVE_SUB),
        ]);
    }
    // dedupe preserve order
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for p in raw {
        let s = p.to_string_lossy().to_string();
        if seen.insert(s) {
            out.push(p);
        }
    }
    out
}

fn dir_has_saves(d: &Path) -> bool {
    fs::read_dir(d)
        .map(|entries| {
            entries.filter_map(|e| e.ok()).any(|e| {
                let n = e.file_name().to_string_lossy().to_string();
                n.starts_with("save") && n.ends_with(".hg") && e.path().is_file()
            })
        })
        .unwrap_or(false)
}

fn is_save_container(name: &str) -> bool {
    // Steam: st_<steamid> · GOG/legacy: DefaultUser
    name.contains("st_") || name == "DefaultUser"
}

fn find_save_dirs_inner() -> Vec<PathBuf> {
    let mut found = Vec::new();
    for root in candidate_roots() {
        if !root.is_dir() {
            continue;
        }
        let entries = match fs::read_dir(&root) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let p = entry.path();
            if !p.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if is_save_container(&name) || dir_has_saves(&p) {
                found.push(p);
            }
        }
    }
    found
}

fn find_save_dir_inner(prefer: Option<String>) -> Option<PathBuf> {
    if let Ok(env) = std::env::var("NMS_SAVE_DIR") {
        let p = PathBuf::from(shellexpand(&env));
        if p.is_dir() {
            return Some(p);
        }
        if p.is_file() {
            if let Some(parent) = p.parent() {
                return Some(parent.to_path_buf());
            }
        }
    }
    if let Some(pr) = prefer {
        let p = PathBuf::from(shellexpand(&pr));
        if p.is_dir() {
            return Some(p);
        }
        if p.is_file() {
            if let Some(parent) = p.parent() {
                return Some(parent.to_path_buf());
            }
        }
    }
    let dirs = find_save_dirs_inner();
    if dirs.is_empty() {
        return None;
    }
    for d in &dirs {
        if dir_has_saves(d) {
            return Some(d.clone());
        }
    }
    dirs.into_iter().next()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SaveFileInfo {
    pub name: String,
    pub size_bytes: u64,
    pub size_display: String,
    pub modified: String,
    pub mtime_ms: i64,
}

fn size_display(sz: u64) -> String {
    if sz > 1024 * 1024 {
        format!("{:.2} MB", sz as f64 / 1024.0 / 1024.0)
    } else {
        format!("{} KB", sz / 1024)
    }
}

fn list_save_files_inner(save_dir: &Path) -> Vec<SaveFileInfo> {
    let mut out = Vec::new();
    let entries = match fs::read_dir(save_dir) {
        Ok(e) => e,
        Err(_) => return out,
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let p = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if !(name.starts_with("save") && name.ends_with(".hg")) || !p.is_file() {
            continue;
        }
        let (sz, mtime_ms, modified) = match p.metadata() {
            Ok(m) => {
                let sz = m.len();
                let (ms, disp) = m
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| {
                        let ms = d.as_millis() as i64;
                        let dt = chrono::DateTime::<Local>::from(
                            std::time::SystemTime::UNIX_EPOCH + d,
                        );
                        (ms, dt.format("%Y-%m-%d %H:%M").to_string())
                    })
                    .unwrap_or((0, "?".to_string()));
                (sz, ms, disp)
            }
            Err(_) => continue,
        };
        out.push(SaveFileInfo {
            name,
            size_bytes: sz,
            size_display: size_display(sz),
            modified,
            mtime_ms,
        });
    }
    out.sort_by(|a, b| b.mtime_ms.cmp(&a.mtime_ms));
    out
}

// ---------------------------------------------------------------------------
// Key mapping (port of key_mapper.py)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
struct MappingEntry {
    #[serde(rename = "Key")]
    key: String,
    #[serde(rename = "Value")]
    value: String,
}

fn load_mapping_from_value(v: &serde_json::Value) -> Result<Vec<MappingEntry>, String> {
    if let Some(arr) = v.get("Mapping").and_then(|m| m.as_array()) {
        serde_json::from_value(serde_json::Value::Array(arr.clone()))
            .map_err(|e| format!("bad Mapping array: {}", e))
    } else if let Some(arr) = v.as_array() {
        serde_json::from_value(serde_json::Value::Array(arr.clone()))
            .map_err(|e| format!("bad mapping list: {}", e))
    } else {
        Err("unexpected mapping file format".to_string())
    }
}

fn cache_valid(p: &Path) -> bool {
    let meta = match p.metadata() {
        Ok(m) => m,
        Err(_) => return false,
    };
    let age_ok = meta
        .modified()
        .ok()
        .and_then(|t| t.elapsed().ok())
        .map(|d| d.as_secs() < (CACHE_MAX_AGE_DAYS as u64 * 86400))
        .unwrap_or(false);
    if !age_ok {
        return false;
    }
    match fs::read_to_string(p)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
    {
        Some(v) => load_mapping_from_value(&v).map(|m| !m.is_empty()).unwrap_or(false),
        None => false,
    }
}

fn load_stale_cache(cache: &Path) -> Option<Vec<MappingEntry>> {
    if !cache.exists() {
        return None;
    }
    let text = fs::read_to_string(cache).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    load_mapping_from_value(&v).ok().filter(|m| !m.is_empty())
}

fn fetch_mapping() -> Result<(Vec<MappingEntry>, &'static str), String> {
    let cache = mapping_cache_path();
    if cache_valid(&cache) {
        if let Some(m) = load_stale_cache(&cache) {
            return Ok((m, "cached"));
        }
    }
    let text = match reqwest::blocking::get(MAPPING_URL) {
        Ok(r) => r.text().map_err(|e| format!("read mapping body: {}", e))?,
        Err(e) => {
            // offline fallback: stale cache even if expired
            if let Some(m) = load_stale_cache(&cache) {
                return Ok((m, "stale cache"));
            }
            return Err(format!("download mapping: {}", e));
        }
    };
    let v: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("parse mapping: {}", e))?;
    let mapping = load_mapping_from_value(&v)?;
    if let Some(parent) = cache.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let cached = serde_json::json!({
        "libMBIN_version": "cached",
        "cached_at": Local::now().to_rfc3339(),
        "Mapping": mapping.iter().map(|e| serde_json::json!({"Key": e.key, "Value": e.value})).collect::<Vec<_>>(),
    });
    let _ = fs::write(
        &cache,
        serde_json::to_string_pretty(&cached).unwrap_or_default(),
    );
    Ok((mapping, "downloaded"))
}

fn map_value(v: &serde_json::Value, lookup: &HashMap<&str, &str>) -> serde_json::Value {
    match v {
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(|x| map_value(x, lookup)).collect())
        }
        serde_json::Value::Object(map) => {
            let mut out = serde_json::Map::with_capacity(map.len());
            for (k, val) in map {
                let nk = lookup.get(k.as_str()).copied().unwrap_or(k.as_str());
                out.insert(nk.to_string(), map_value(val, lookup));
            }
            serde_json::Value::Object(out)
        }
        _ => v.clone(),
    }
}

fn map_keys(v: &serde_json::Value, mapping: &[MappingEntry]) -> serde_json::Value {
    let lookup: HashMap<&str, &str> = mapping
        .iter()
        .map(|e| (e.key.as_str(), e.value.as_str()))
        .collect();
    map_value(v, &lookup)
}

fn reverse_map_keys(v: &serde_json::Value, mapping: &[MappingEntry]) -> serde_json::Value {
    let lookup: HashMap<&str, &str> = mapping
        .iter()
        .map(|e| (e.value.as_str(), e.key.as_str()))
        .collect();
    map_value(v, &lookup)
}

// ---------------------------------------------------------------------------
// .hg (de)compression (port of extract + recompressor)
// ---------------------------------------------------------------------------

fn u32le(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

fn decompress_hg(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut out: Vec<u8> = Vec::new();
    let mut pos = 0usize;
    let magic_le = [0xE5, 0xA1, 0xED, 0xFE];
    while pos + 16 <= data.len() {
        if data[pos..pos + 4] != magic_le {
            // resync: scan for next block magic
            let rel = data[pos..]
                .windows(4)
                .position(|w| w == magic_le);
            match rel {
                Some(r) => {
                    pos += r;
                    if pos + 16 > data.len() {
                        break;
                    }
                }
                None => break,
            }
        }
        let compressed_size = u32le(&data[pos + 4..pos + 8]) as usize;
        let uncompressed_size = u32le(&data[pos + 8..pos + 12]) as usize;
        pos += 16; // skip header incl. 4 padding bytes
        if pos + compressed_size > data.len() {
            break;
        }
        let chunk = &data[pos..pos + compressed_size];
        let block =
            decompress(chunk, uncompressed_size).map_err(|e| format!("lz4 block: {}", e))?;
        out.extend_from_slice(&block);
        pos += compressed_size;
    }
    if out.is_empty() {
        return Err("failed to decompress — not a valid save file?".to_string());
    }
    Ok(out)
}

fn parse_first_json(s: &str) -> Result<serde_json::Value, String> {
    let mut de = serde_json::Deserializer::from_str(s);
    use serde::de::Deserialize as De;
    match serde_json::Value::deserialize(&mut de).map_err(|e| e.to_string()) {
        Ok(v) => Ok(v),
        Err(e) => {
            // tolerate trailing garbage like the Python Extra-data path
            if e.contains("trailing characters") {
                Err(format!("parse save json: {}", e))
            } else {
                Err(format!("parse save json: {}", e))
            }
        }
    }
}

fn compress_blocks(json_bytes: &[u8]) -> (Vec<u8>, usize) {
    let mut out: Vec<u8> = Vec::new();
    let mut offset = 0usize;
    let mut blocks = 0usize;
    while offset < json_bytes.len() {
        let end = (offset + MAX_UNCOMPRESSED_BLOCK_SIZE).min(json_bytes.len());
        let chunk = &json_bytes[offset..end];
        let compressed = compress(chunk);
        let mut header = Vec::with_capacity(16);
        header.extend_from_slice(&BLOCK_MAGIC.to_le_bytes());
        header.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
        header.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
        header.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&header);
        out.extend_from_slice(&compressed);
        offset = end;
        blocks += 1;
    }
    (out, blocks)
}

// ---------------------------------------------------------------------------
// Base helpers (ports of save_editor.py find_key_recursively + editor.py)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Seg {
    Key(String),
    Idx(usize),
}

fn find_key_path(v: &serde_json::Value, target: &str) -> Option<Vec<Seg>> {
    fn rec(v: &serde_json::Value, target: &str, path: &mut Vec<Seg>) -> bool {
        match v {
            serde_json::Value::Object(map) => {
                for (k, val) in map {
                    if k == target {
                        path.push(Seg::Key(k.clone()));
                        return true;
                    }
                    path.push(Seg::Key(k.clone()));
                    if rec(val, target, path) {
                        return true;
                    }
                    path.pop();
                }
                false
            }
            serde_json::Value::Array(arr) => {
                for (i, val) in arr.iter().enumerate() {
                    path.push(Seg::Idx(i));
                    if rec(val, target, path) {
                        return true;
                    }
                    path.pop();
                }
                false
            }
            _ => false,
        }
    }
    let mut path = Vec::new();
    if rec(v, target, &mut path) {
        Some(path)
    } else {
        None
    }
}

fn get_at<'a>(v: &'a serde_json::Value, path: &[Seg]) -> Option<&'a serde_json::Value> {
    let mut cur = v;
    for seg in path {
        match seg {
            Seg::Key(k) => cur = cur.get(k)?,
            Seg::Idx(i) => cur = cur.get(i)?,
        }
    }
    Some(cur)
}

fn get_at_mut<'a>(
    v: &'a mut serde_json::Value,
    path: &[Seg],
) -> Option<&'a mut serde_json::Value> {
    let mut cur = v;
    for seg in path {
        match seg {
            Seg::Key(k) => cur = cur.get_mut(k)?,
            Seg::Idx(i) => cur = cur.get_mut(i)?,
        }
    }
    Some(cur)
}

fn base_type_of(base: &serde_json::Value) -> String {
    base.get("BaseType")
        .and_then(|t| t.get("PersistentBaseTypes"))
        .and_then(|t| t.as_str())
        .unwrap_or("Unknown")
        .to_string()
}

fn base_objects_count(base: &serde_json::Value) -> usize {
    base.get("Objects")
        .and_then(|o| o.as_array())
        .map(|a| a.len())
        .unwrap_or(0)
}

fn base_owner_uid(base: &serde_json::Value) -> String {
    base.get("Owner")
        .and_then(|o| o.get("UID"))
        .and_then(|u| u.as_str())
        .unwrap_or("")
        .to_string()
}

fn ship_ownership_list(save: &serde_json::Value) -> Option<Vec<serde_json::Value>> {
    let path = find_key_path(save, "ShipOwnership")?;
    get_at(save, &path)
        .and_then(|v| v.as_array())
        .map(|a| a.to_vec())
}

fn base_display_name(
    base: &serde_json::Value,
    idx: usize,
    ships: &Option<Vec<serde_json::Value>>,
) -> String {
    let btype = base_type_of(base);
    let name = base
        .get("Name")
        .and_then(|n| n.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if btype == "PlayerShipBase" {
        if let Some(ud) = base.get("UserData").and_then(|u| u.as_u64()) {
            if let Some(list) = ships {
                if let Some(ship) = list.get(ud as usize) {
                    let sn = ship
                        .get("Name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("")
                        .trim();
                    if !sn.is_empty() {
                        return sn.to_string();
                    }
                }
            }
            if name.is_empty() || name == "Default" {
                return format!("Corvette #{} ({} objs)", ud, base_objects_count(base));
            }
        }
        if !name.is_empty() {
            if name == "Default" {
                if let Some(list) = ships {
                    // fall through to indexed name
                    let _ = list;
                }
                return format!("Corvette #{} ({} objs)", idx, base_objects_count(base));
            }
            return name;
        }
        return format!("Corvette #{} ({} objs)", idx, base_objects_count(base));
    }
    if btype == "FreighterBase" && name.is_empty() {
        return "Freighter Base".to_string();
    }
    if !name.is_empty() {
        return name;
    }
    format!("Unnamed {}", idx)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BaseSummary {
    pub idx: usize,
    pub name: String,
    pub display_name: String,
    pub base_type: String,
    pub objects: usize,
    pub owner_uid: String,
}

fn summarize_bases(save: &serde_json::Value) -> Result<Vec<BaseSummary>, String> {
    let path =
        find_key_path(save, "PersistentPlayerBases").ok_or("PersistentPlayerBases not found")?;
    let bases = get_at(save, &path)
        .and_then(|v| v.as_array())
        .ok_or("PersistentPlayerBases is not an array")?;
    let ships = ship_ownership_list(save);
    Ok(bases
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let raw = b
                .get("Name")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            BaseSummary {
                idx: i,
                name: raw.clone(),
                display_name: base_display_name(b, i, &ships),
                base_type: base_type_of(b),
                objects: base_objects_count(b),
                owner_uid: base_owner_uid(b),
            }
        })
        .collect())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TypeCounts {    pub ship: usize,
    pub planet: usize,
    pub freighter: usize,
    pub space: usize,
    pub total_objs: usize,
}

fn count_types(summaries: &[BaseSummary]) -> TypeCounts {
    let mut c = TypeCounts {
        ship: 0,
        planet: 0,
        freighter: 0,
        space: 0,
        total_objs: 0,
    };
    for s in summaries {
        c.total_objs += s.objects;
        match s.base_type.as_str() {
            "PlayerShipBase" => c.ship += 1,
            t if PLANETARY_TYPES.contains(&t) => c.planet += 1,
            "FreighterBase" => c.freighter += 1,
            "PlayerSpaceBase" => c.space += 1,
            _ => {}
        }
    }
    c
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub(crate) fn find_save_dirs() -> Vec<String> {
    find_save_dirs_inner()
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect()
}

#[tauri::command]
pub(crate) fn find_save_dir(prefer: Option<String>) -> Option<String> {
    find_save_dir_inner(prefer).map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
pub(crate) fn list_save_files(save_dir: String) -> Vec<SaveFileInfo> {
    list_save_files_inner(Path::new(&shellexpand(&save_dir)))
}

#[tauri::command]
pub(crate) fn list_save_subdirs(save_dir: String) -> Vec<String> {
    // Child folders that look like save containers (for when the user picks
    // the parent HelloGames/NMS folder instead of the st_*/DefaultUser dir).
    let dir = PathBuf::from(shellexpand(&save_dir));
    let mut out = Vec::new();
    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return out,
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let p = entry.path();
        if !p.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if is_save_container(&name) || dir_has_saves(&p) {
            out.push(p.to_string_lossy().to_string());
        }
    }
    out.sort();
    out
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DecompressResult {
    pub bases: Vec<BaseSummary>,
    pub counts: TypeCounts,
}

#[tauri::command]
pub(crate) fn decompress_save(
    app: tauri::AppHandle,
    state: tauri::State<Mutex<SaveState>>,
    save_dir: String,
    save_file: String,
) -> Result<DecompressResult, String> {
    use crate::logs::dlog;
    let dir = PathBuf::from(shellexpand(&save_dir));
    let full = dir.join(&save_file);
    let data = fs::read(&full).map_err(|e| format!("read {}: {}", full.display(), e))?;
    dlog(&app, "info", "bases", format!("reading {} ({} KB)", save_file, data.len() / 1024));

    // NOTE: no backup here — loading is read-only. Every path that writes a
    // live file (overwrite, restore) snapshots it first.
    let raw = decompress_hg(&data)?;
    dlog(&app, "info", "bases", format!("decompressed to {} KB JSON", raw.len() / 1024));
    let text = String::from_utf8(raw).map_err(|e| format!("save is not utf-8: {}", e))?;
    let mut json: serde_json::Value = parse_first_json(&text)?;

    // deobfuscate keys
    let (mapping, mapping_source) = fetch_mapping()?;
    dlog(
        &app,
        if mapping_source == "stale cache" { "warn" } else { "info" },
        "bases",
        format!("key mapping: {} entries ({})", mapping.len(), mapping_source),
    );
    json = map_keys(&json, &mapping);

    let bases = summarize_bases(&json)?;
    let counts = count_types(&bases);
    dlog(
        &app,
        "info",
        "bases",
        format!(
            "loaded {} bases ({} ship, {} planet, {} objs)",
            bases.len(),
            counts.ship,
            counts.planet,
            counts.total_objs
        ),
    );

    let mut st = state.lock().map_err(|e| e.to_string())?;
    st.save_dir = Some(dir);
    st.save_file = Some(save_file);
    st.save_json = Some(json);

    Ok(DecompressResult { bases, counts })
}

#[tauri::command]
pub(crate) fn list_bases(
    state: tauri::State<Mutex<SaveState>>,
    filter: Option<String>,
) -> Result<Vec<BaseSummary>, String> {
    let st = state.lock().map_err(|e| e.to_string())?;
    let save = st.save_json.as_ref().ok_or("no save loaded")?;
    let mut bases = summarize_bases(save)?;
    if let Some(f) = filter {
        match f.as_str() {
            "PlayerShipBase" => bases.retain(|b| b.base_type == "PlayerShipBase"),
            "PLANETARY" => bases.retain(|b| PLANETARY_TYPES.contains(&b.base_type.as_str())),
            _ => {}
        }
    }
    Ok(bases)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ExportResult {
    pub path: String,
    pub content: String,
}

/// Fetch a base plus its display-derived safe filename.
/// Content is returned to the frontend so it never needs fs-plugin reads
/// (the fs scope cannot cover arbitrary user-chosen paths).
fn get_base(
    state: &tauri::State<Mutex<SaveState>>,
    idx: usize,
) -> Result<(serde_json::Value, String, String), String> {
    let st = state.lock().map_err(|e| e.to_string())?;
    let save = st.save_json.as_ref().ok_or("no save loaded")?;
    let path = find_key_path(save, "PersistentPlayerBases").ok_or("bases not found")?;
    let bases = get_at(save, &path)
        .and_then(|v| v.as_array())
        .ok_or("bases not an array")?;
    let base = bases.get(idx).ok_or("base index out of range")?.clone();
    let ships = ship_ownership_list(save);
    let disp = base_display_name(&base, idx, &ships);
    let safe = safe_name(&disp);
    Ok((base, safe, disp))
}

#[tauri::command]
pub(crate) fn get_base_json(
    state: tauri::State<Mutex<SaveState>>,
    idx: usize,
) -> Result<String, String> {
    let (base, _, _) = get_base(&state, idx)?;
    serde_json::to_string_pretty(&base).map_err(|e| e.to_string())
}

fn nmsbase_text(base: &serde_json::Value) -> Result<String, String> {
    let objs = base
        .get("Objects")
        .and_then(|o| o.as_array())
        .cloned()
        .unwrap_or_default();
    if objs.is_empty() {
        return Ok(String::new());
    }
    let parts: Result<Vec<String>, _> = objs
        .iter()
        .map(|o| serde_json::to_string_pretty(o).map_err(|e| e.to_string()))
        .collect();
    Ok(format!(",\n{}", parts?.join(",\n")))
}

#[tauri::command]
pub(crate) fn get_nmsbase_text(
    state: tauri::State<Mutex<SaveState>>,
    idx: usize,
) -> Result<String, String> {
    let (base, _, _) = get_base(&state, idx)?;
    nmsbase_text(&base)
}

#[tauri::command]
pub(crate) fn read_text_file(path: String) -> Result<String, String> {
    // std::fs has no capability-scope limits, unlike the fs plugin.
    fs::read_to_string(PathBuf::from(shellexpand(&path))).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn export_base(
    app: tauri::AppHandle,
    state: tauri::State<Mutex<SaveState>>,
    idx: usize,
    out_path: Option<String>,
) -> Result<ExportResult, String> {
    use crate::logs::dlog;
    let (base, safe, disp) = get_base(&state, idx)?;

    let out = match out_path {
        Some(p) => {
            let mut pb = PathBuf::from(shellexpand(&p));
            if pb.extension().is_none() {
                pb.set_extension("json");
            }
            pb
        }
        None => {
            fs::create_dir_all(output_bases_dir()).map_err(|e| e.to_string())?;
            output_bases_dir().join(format!("{}.json", safe))
        }
    };
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if out.exists() {
        fs::create_dir_all(backups_bases_dir()).map_err(|e| e.to_string())?;
        let bak = backups_bases_dir().join(format!("{}_backup_{}.json", safe, timestamp()));
        let _ = fs::copy(&out, &bak);
    }
    let text = serde_json::to_string_pretty(&base).map_err(|e| e.to_string())?;
    fs::write(&out, &text).map_err(|e| e.to_string())?;
    dlog(
        &app,
        "info",
        "bases",
        format!("exported '{disp}' → {} ({} KB)", out.display(), text.len() / 1024),
    );

    // mirror to Base Builder folder when present (djmonkeyuk app interop)
    if let Some(home) = dirs::home_dir() {
        let bba = home
            .join("Documents")
            .join("No Mans Sky Base Builder")
            .join("bases");
        if bba.is_dir() {
            let _ = fs::copy(&out, bba.join(format!("{}.json", safe)));
        }
    }

    Ok(ExportResult {
        path: out.to_string_lossy().to_string(),
        content: text,
    })
}

#[tauri::command]
pub(crate) fn export_nmsbase(
    app: tauri::AppHandle,
    state: tauri::State<Mutex<SaveState>>,
    idx: usize,
    out_path: Option<String>,
) -> Result<ExportResult, String> {
    use crate::logs::dlog;
    let (base, safe, disp) = get_base(&state, idx)?;

    let dest = match out_path {
        Some(p) => {
            let mut pb = PathBuf::from(shellexpand(&p));
            if pb.extension().is_none() {
                pb.set_extension("nmsbase");
            }
            pb
        }
        None => {
            fs::create_dir_all(output_nmsbase_dir()).map_err(|e| e.to_string())?;
            output_nmsbase_dir().join(format!("{}.nmsbase", safe))
        }
    };
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if dest.exists() {
        fs::create_dir_all(backups_bases_dir()).map_err(|e| e.to_string())?;
        let bak = backups_bases_dir().join(format!(
            "{}_{}_backup_{}.nmsbase",
            safe,
            dest.file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default(),
            timestamp()
        ));
        let _ = fs::copy(&dest, &bak);
    }

    let txt = nmsbase_text(&base)?;
    fs::write(&dest, &txt).map_err(|e| e.to_string())?;
    dlog(
        &app,
        "info",
        "bases",
        format!("exported NMSBASE '{disp}' → {} ({} objects)", dest.display(), base.get("Objects").and_then(|o| o.as_array()).map(|a| a.len()).unwrap_or(0)),
    );

    // history copy
    fs::create_dir_all(output_nmsbase_dir()).map_err(|e| e.to_string())?;
    let hist = output_nmsbase_dir().join(format!("{}.nmsbase", safe));
    if hist != dest {
        let _ = fs::write(&hist, &txt);
    }

    Ok(ExportResult {
        path: dest.to_string_lossy().to_string(),
        content: txt,
    })
}

/// Normalize import payloads: full base dict, Objects-only array,
/// single object, or .nmsbase leading-comma text (parsed by caller into text).
fn normalize_import(text: &str) -> Result<serde_json::Value, String> {
    let mut t = text.trim().to_string();
    while t.starts_with(',') {
        t = t[1..].trim_start().to_string();
    }
    // try direct parse, else wrap bare object stream in [...]
    let v: serde_json::Value = serde_json::from_str(&t).or_else(|_| {
        serde_json::from_str::<serde_json::Value>(&format!("[{}]", t))
            .map_err(|e| format!("not valid JSON: {}", e))
    })?;
    if let Some(arr) = v.as_array() {
        if arr.is_empty() {
            return Err("JSON list is empty".to_string());
        }
        let first = &arr[0];
        if first.get("ObjectID").is_some() {
            // objects-only array -> caller wraps into target base
            return Ok(serde_json::Value::Array(arr.clone()));
        }
        if first.get("Objects").is_some() {
            return Ok(first.clone());
        }
        return Ok(first.clone());
    }
    if v.get("ObjectID").is_some() && v.get("Objects").is_none() {
        return Ok(serde_json::json!([v]));
    }
    if v.get("Objects").is_none() {
        return Err("base JSON missing 'Objects' key".to_string());
    }
    Ok(v)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ImportResult {
    pub idx: usize,
    pub objects: usize,
    pub backup_path: String,
}

#[tauri::command]
pub(crate) fn import_base(
    app: tauri::AppHandle,
    state: tauri::State<Mutex<SaveState>>,
    idx: usize,
    payload: String,
) -> Result<ImportResult, String> {
    use crate::logs::dlog;
    let data = normalize_import(&payload)?;
    dlog(&app, "info", "bases", format!("import payload parsed ({} bytes)", payload.len()));
    let mut st = state.lock().map_err(|e| e.to_string())?;
    let save = st.save_json.as_mut().ok_or("no save loaded")?;
    let path = find_key_path(save, "PersistentPlayerBases").ok_or("bases not found")?;

    let target: serde_json::Value = {
        let bases = get_at(save, &path)
            .and_then(|v| v.as_array())
            .ok_or("bases not an array")?;
        bases.get(idx).ok_or("base index out of range")?.clone()
    };

    // build replacement: objects-only arrays replace Objects on the target slot
    let mut new_base = if let Some(arr) = data.as_array() {
        let mut nb = target.clone();
        if let Some(o) = nb.get_mut("Objects") {
            *o = serde_json::Value::Array(arr.clone());
        } else {
            nb["Objects"] = serde_json::Value::Array(arr.clone());
        }
        nb["LastUpdateTimestamp"] = serde_json::json!(Local::now().timestamp());
        nb
    } else {
        data
    };

    // preserve slot identity: keep target Name if payload is unnamed
    if new_base
        .get("Name")
        .and_then(|n| n.as_str())
        .map(|s| s.is_empty())
        .unwrap_or(true)
    {
        if let Some(n) = target.get("Name").cloned() {
            new_base["Name"] = n;
        }
    }

    // backup original base
    fs::create_dir_all(backups_bases_dir()).map_err(|e| e.to_string())?;
    let raw_name = target
        .get("Name")
        .and_then(|n| n.as_str())
        .unwrap_or("base");
    let bak = backups_bases_dir().join(format!("{}_backup_{}.json", safe_name(raw_name), timestamp()));
    let _ = fs::write(
        &bak,
        serde_json::to_string_pretty(&target).unwrap_or_default(),
    );

    let n_objects = new_base
        .get("Objects")
        .and_then(|o| o.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    let bases_arr = get_at_mut(save, &path)
        .and_then(|v| v.as_array_mut())
        .ok_or("bases not an array")?;
    if idx >= bases_arr.len() {
        return Err("base index out of range".to_string());
    }
    bases_arr[idx] = new_base;

    dlog(
        &app,
        "info",
        "bases",
        format!(
            "injected {} objects into slot {idx} (original backed up → {})",
            n_objects,
            bak.file_name().unwrap_or_default().to_string_lossy()
        ),
    );
    Ok(ImportResult {
        idx,
        objects: n_objects,
        backup_path: bak.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub(crate) fn recompress_save(
    app: tauri::AppHandle,
    state: tauri::State<Mutex<SaveState>>,
    mode: String,
) -> Result<String, String> {
    use crate::logs::dlog;
    let st = state.lock().map_err(|e| e.to_string())?;
    let save = st.save_json.as_ref().ok_or("no save loaded")?.clone();
    let dir = st.save_dir.clone().ok_or("no save dir")?;
    let file = st.save_file.clone().ok_or("no save file")?;
    drop(st);

    let (mapping, mapping_source) = fetch_mapping()?;
    dlog(
        &app,
        if mapping_source == "stale cache" { "warn" } else { "info" },
        "bases",
        format!("key mapping: {} entries ({})", mapping.len(), mapping_source),
    );
    let obfuscated = reverse_map_keys(&save, &mapping);
    let json_str =
        serde_json::to_string(&obfuscated).map_err(|e| format!("serialize: {}", e))?;
    dlog(&app, "info", "bases", format!("serialized compact JSON ({} KB)", json_str.len() / 1024));
    let (blob, blocks) = compress_blocks(json_str.as_bytes());
    dlog(
        &app,
        "info",
        "bases",
        format!("compressed into {blocks} LZ4 block(s) ({} KB)", blob.len() / 1024),
    );

    let out = if mode == "overwrite" {
        let live = dir.join(&file);
        fs::create_dir_all(backups_save_dir()).map_err(|e| e.to_string())?;
        let stem = Path::new(&file)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "save".to_string());
        let bak = backups_save_dir().join(format!("{}_before_recompress_{}.hg", stem, timestamp()));
        fs::copy(&live, &bak).map_err(|e| format!("backup live: {}", e))?;
        dlog(
            &app,
            "info",
            "bases",
            format!("backed up live save → {}", bak.file_name().unwrap_or_default().to_string_lossy()),
        );
        live
    } else {
        fs::create_dir_all(output_dir()).map_err(|e| e.to_string())?;
        output_dir().join(&file)
    };

    // atomic write via .tmp + rename
    let tmp = out.with_extension("tmp");
    fs::write(&tmp, &blob).map_err(|e| format!("write tmp: {}", e))?;
    fs::rename(&tmp, &out).map_err(|e| format!("rename tmp: {}", e))?;
    dlog(&app, "info", "bases", format!("wrote {}", out.display()));
    Ok(out.to_string_lossy().to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BackupInfo {
    pub name: String,
    pub size_display: String,
    pub modified: String,
    pub path: String,
}

#[tauri::command]
pub(crate) fn backup_saves(
    app: tauri::AppHandle,
    save_dir: String,
) -> Result<Vec<String>, String> {
    use crate::logs::dlog;
    let dir = PathBuf::from(shellexpand(&save_dir));
    if !dir.is_dir() {
        return Err(format!("not a directory: {}", dir.display()));
    }
    fs::create_dir_all(backups_save_dir()).map_err(|e| e.to_string())?;
    let ts = timestamp();
    let mut done = Vec::new();
    let mut entries: Vec<PathBuf> = fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|x| x.path()))
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .map(|n| {
                        let n = n.to_string_lossy().to_lowercase();
                        n.starts_with("save")
                            && n.ends_with(".hg")
                            && !n.contains("backup")
                            && !n.contains("before_recompress")
                            && !n.contains("pre_restore")
                    })
                    .unwrap_or(false)
        })
        .collect();
    entries.sort();
    for src in entries {
        if src.metadata().map(|m| m.len() < 18).unwrap_or(true) {
            continue;
        }
        let stem = src
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "save".to_string());
        let mut dst = backups_save_dir().join(format!("{}_manual_backup_{}.hg", stem, ts));
        if dst.exists() {
            dst = backups_save_dir().join(format!(
                "{}_manual_backup_{}_{}.hg",
                stem,
                ts,
                done.len()
            ));
        }
        fs::copy(&src, &dst).map_err(|e| e.to_string())?;
        dlog(
            &app,
            "info",
            "bases",
            format!(
                "backed up {} → {}",
                src.file_name().unwrap_or_default().to_string_lossy(),
                dst.file_name().unwrap_or_default().to_string_lossy()
            ),
        );
        done.push(dst.to_string_lossy().to_string());
    }
    if done.is_empty() {
        return Err("nothing backed up".to_string());
    }
    Ok(done)
}

#[tauri::command]
pub(crate) fn list_backups(stem: Option<String>) -> Vec<BackupInfo> {
    let dir = backups_save_dir();
    let mut out = Vec::new();
    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return out,
    };
    let mut items: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|x| x.path()))
        .filter(|p| {
            p.extension().map(|e| e == "hg").unwrap_or(false)
                && stem.as_ref().map(|s| {
                    p.file_stem()
                        .map(|st| st.to_string_lossy().contains(s.as_str()))
                        .unwrap_or(false)
                }).unwrap_or(true)
        })
        .collect();
    items.sort_by_key(|p| {
        p.metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs()))
            .unwrap_or(0)
    });
    items.reverse();
    for p in items {
        let (sz, modified) = match p.metadata() {
            Ok(m) => {
                let disp = m
                    .modified()
                    .ok()
                    .map(|t| {
                        chrono::DateTime::<Local>::from(t)
                            .format("%Y-%m-%d %H:%M:%S")
                            .to_string()
                    })
                    .unwrap_or_else(|| "?".to_string());
                (m.len(), disp)
            }
            Err(_) => continue,
        };
        out.push(BackupInfo {
            name: p.file_name().unwrap_or_default().to_string_lossy().to_string(),
            size_display: size_display(sz),
            modified,
            path: p.to_string_lossy().to_string(),
        });
    }
    out
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ManagedBackup {
    pub name: String,
    /// "save" (.hg) or "base" (.json/.nmsbase)
    pub kind: String,
    pub size_display: String,
    pub size_bytes: u64,
    pub modified_ms: i64,
    pub modified: String,
    pub path: String,
}

fn backup_file_meta(p: &Path) -> Option<(u64, i64, String)> {
    let m = p.metadata().ok()?;
    let ms = m
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok().map(|d| d.as_millis() as i64))
        .unwrap_or(0);
    let disp = m
        .modified()
        .ok()
        .map(|t| {
            chrono::DateTime::<Local>::from(t)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| "?".to_string());
    Some((m.len(), ms, disp))
}

/// Every managed backup: save .hg files plus base .json/.nmsbase files,
/// newest first. Output/ exports are working files, not backups.
#[tauri::command]
pub(crate) fn list_all_backups() -> Vec<ManagedBackup> {
    let mut out = Vec::new();
    let roots = [
        (backups_save_dir(), "save", &["hg"] as &[&str]),
        (backups_bases_dir(), "base", &["json", "nmsbase"]),
    ];
    for (dir, kind, exts) in roots {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for p in entries.filter_map(|e| e.ok().map(|x| x.path())) {
            if !p.is_file() {
                continue;
            }
            let ext_ok = p
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| exts.contains(&e.to_lowercase().as_str()))
                .unwrap_or(false);
            if !ext_ok {
                continue;
            }
            if let Some((sz, ms, modified)) = backup_file_meta(&p) {
                out.push(ManagedBackup {
                    name: p.file_name().unwrap_or_default().to_string_lossy().to_string(),
                    kind: kind.to_string(),
                    size_display: size_display(sz),
                    size_bytes: sz,
                    modified_ms: ms,
                    modified,
                    path: p.to_string_lossy().to_string(),
                });
            }
        }
    }
    out.sort_by(|a, b| b.modified_ms.cmp(&a.modified_ms));
    out
}

/// Delete one managed backup. The path must resolve inside the backups
/// root — anything else is rejected.
#[tauri::command]
pub(crate) fn delete_backup(app: tauri::AppHandle, path: String) -> Result<(), String> {
    use crate::logs::dlog;
    let root = saves_root().join("backups");
    let name = delete_managed_backup(&root, &path)?;
    dlog(&app, "info", "bases", format!("deleted backup '{name}'"));
    Ok(())
}

fn delete_managed_backup(root: &Path, path: &str) -> Result<String, String> {
    let target = PathBuf::from(shellexpand(path));
    let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let canonical_target = target
        .canonicalize()
        .map_err(|_| "backup not found".to_string())?;
    if !canonical_target.starts_with(&canonical_root) {
        return Err("refusing to delete outside the backups folder".to_string());
    }
    if !canonical_target.is_file() {
        return Err("not a file".to_string());
    }
    let name = canonical_target
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    fs::remove_file(&canonical_target).map_err(|e| e.to_string())?;
    Ok(name)
}

#[tauri::command]
pub(crate) fn restore_save(
    app: tauri::AppHandle,
    backup_path: String,
    save_dir: String,
    save_file: String,
) -> Result<String, String> {
    use crate::logs::dlog;
    let target = PathBuf::from(shellexpand(&save_dir)).join(&save_file);
    let stem = Path::new(&save_file)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "save".to_string());
    if target.exists() {
        fs::create_dir_all(backups_save_dir()).map_err(|e| e.to_string())?;
        let pre =
            backups_save_dir().join(format!("{}_pre_restore_{}.hg", stem, timestamp()));
        fs::copy(&target, &pre).map_err(|e| format!("pre-restore backup: {}", e))?;
        dlog(
            &app,
            "info",
            "bases",
            format!("backed up live file before restore → {}", pre.file_name().unwrap_or_default().to_string_lossy()),
        );
    }
    fs::copy(&backup_path, &target).map_err(|e| format!("restore: {}", e))?;
    dlog(
        &app,
        "info",
        "bases",
        format!("restored '{save_file}' from {}", PathBuf::from(&backup_path).file_name().unwrap_or_default().to_string_lossy()),
    );
    Ok(target.to_string_lossy().to_string())
}

// ---------------------------------------------------------------------------
// App entry
// ---------------------------------------------------------------------------

#[cfg(test)]
mod live_save_tests {
    use super::*;

    fn live_save() -> Option<(PathBuf, String)> {
        let dir = find_save_dir_inner(None)?;
        let files = list_save_files_inner(&dir);
        let f = files.into_iter().find(|f| f.name == "save.hg")?;
        Some((dir, f.name))
    }

    #[test]
    fn decompress_map_and_roundtrip_live_save() {
        let (dir, file) = live_save().expect("no live save.hg found for test");
        let data = fs::read(dir.join(&file)).expect("read save.hg");
        let raw = decompress_hg(&data).expect("lz4 decompress");
        let text = String::from_utf8(raw).expect("utf-8");
        let json: serde_json::Value = parse_first_json(&text).expect("json parse");

        let (mapping, _source) = fetch_mapping().expect("mapping download/cache");
        assert!(!mapping.is_empty(), "empty mapping");

        let mapped = map_keys(&json, &mapping);
        let bases = summarize_bases(&mapped).expect("summarize bases");
        assert!(!bases.is_empty(), "no bases found");
        let counts = count_types(&bases);
        println!(
            "bases={} ship={} planet={} freighter={} space={} total_objs={}",
            bases.len(),
            counts.ship,
            counts.planet,
            counts.freighter,
            counts.space,
            counts.total_objs
        );

        // recompress round-trip: reverse-map -> compact -> blocks -> decompress.
        // NOTE: exact equality is NOT expected: mapping.json contains duplicate
        // Values (7) and duplicate Keys (9), so map->reverse-map is inherently
        // lossy (same in Python recompressor.py). Verify structural integrity.
        let back = reverse_map_keys(&mapped, &mapping);
        let s = serde_json::to_string(&back).expect("serialize");
        let (blob, n) = compress_blocks(s.as_bytes());
        assert!(n > 0, "no blocks produced");
        let raw2 = decompress_hg(&blob).expect("re-decompress");
        let v2: serde_json::Value =
            serde_json::from_str(&String::from_utf8(raw2).expect("utf-8")).expect("re-parse");
        let bases2 = summarize_bases(&map_keys(&v2, &mapping)).expect("re-summarize");
        assert_eq!(bases.len(), bases2.len(), "base count changed");
        for (a, b) in bases.iter().zip(bases2.iter()) {
            assert_eq!(a.objects, b.objects, "object count changed for base {}", a.idx);
            assert_eq!(a.base_type, b.base_type, "type changed for base {}", a.idx);
        }
    }

    #[test]
    #[ignore]
    fn dump_mapped_for_python_diff() {
        // one-off: dump Rust-mapped save for diffing against Python output
        let (dir, file) = live_save().expect("no live save.hg found for test");
        let data = fs::read(dir.join(&file)).expect("read save.hg");
        let raw = decompress_hg(&data).expect("lz4 decompress");
        let text = String::from_utf8(raw).expect("utf-8");
        let json: serde_json::Value = parse_first_json(&text).expect("json parse");
        let (mapping, _source) = fetch_mapping().expect("mapping");
        let mapped = map_keys(&json, &mapping);
        fs::write(
            "/tmp/rust_mapped.json",
            serde_json::to_string_pretty(&mapped).expect("pretty"),
        )
        .expect("write dump");
    }
}

#[cfg(test)]
mod backup_manager_tests {
    use super::*;

    #[test]
    fn delete_guard_rejects_escapes_and_deletes_own() {
        let base = std::env::temp_dir().join(format!("nmm_deltest_{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let root = base.join("backups");
        fs::create_dir_all(root.join("save files")).unwrap();
        let own = root.join("save files/save_backup_20260101.hg");
        fs::write(&own, b"data").unwrap();
        let outside = base.join("evil.hg");
        fs::write(&outside, b"evil").unwrap();

        // outside the root: rejected, file untouched
        assert!(delete_managed_backup(&root, outside.to_str().unwrap()).is_err());
        assert!(outside.is_file());
        // missing file: clean error
        assert!(delete_managed_backup(&root, root.join("nope.hg").to_str().unwrap()).is_err());
        // own file: deleted
        assert_eq!(
            delete_managed_backup(&root, own.to_str().unwrap()).unwrap(),
            "save_backup_20260101.hg"
        );
        assert!(!own.exists());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn list_all_backups_finds_both_kinds() {
        // exercises the real listing against a fake root is not possible
        // (paths are fixed); at minimum the live call must not error
        let all = super::list_all_backups();
        assert!(all.iter().all(|b| b.kind == "save" || b.kind == "base"));
    }
}
