<script lang="ts">
  import { onMount } from "svelte";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { api } from "$lib/api";
  import type { BackupInfo, BaseSummary, SaveFileInfo, TypeCounts } from "$lib/types";
  import { clearSaveDirOverride, loadBasesSort, loadSaveDirOverride, saveBasesSort, saveSaveDirOverride } from "$lib/bases-settings";
  import DataList from "$lib/components/data-list.svelte";
  import type { BasesSortDir, BasesSortKey } from "$lib/bases-settings";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Badge } from "$lib/components/ui/badge";
  import * as Dialog from "$lib/components/ui/dialog";
  import * as Table from "$lib/components/ui/table";
  import { Textarea } from "$lib/components/ui/textarea";
  import { Separator } from "$lib/components/ui/separator";
  import * as Select from "$lib/components/ui/select";
  import {
    CircleAlert,
    CircleCheck,
    Copy,
    Database,
    Download,
    Eye,
    FileBraces,
    FileBracesCorner,
    FolderOpen,
    HardDriveDownload,
    Info,
    LoaderCircle,
    Save,
    Search,
    Settings,
    Upload,
    X,
  } from "lucide-svelte";

  // --- saves ---
  let saveDir: string | null = $state(null);
  let saveDirManual = $state(false);
  let saveFiles: SaveFileInfo[] = $state([]);
  let selectedSave: string | null = $state(null);
  let loading = $state(false);

  // --- bases ---
  let bases: BaseSummary[] = $state([]);
  let filter: "Both" | "Corvettes" | "Planetary" = $state("Both");
  let counts: TypeCounts | null = $state(null);
  let selectedBase: number | null = $state(null);
  let search = $state("");
  let sortKey: BasesSortKey = $state("name");
  let sortDir: BasesSortDir = $state("asc");

  // --- status + toasts ---
  let status = $state("Ready. Autodetecting Proton dir…");
  interface Toast {
    id: number;
    kind: "success" | "error" | "info";
    msg: string;
  }
  let toasts: Toast[] = $state([]);
  let toastId = 0;
  function pushToast(kind: Toast["kind"], msg: string) {
    const id = ++toastId;
    toasts = [...toasts, { id, kind, msg }];
    status = msg;
    setTimeout(() => {
      toasts = toasts.filter((t) => t.id !== id);
    }, 6000);
  }
  const toastOk = (m: string) => pushToast("success", m);
  const toastErr = (m: string) => pushToast("error", m);

  // --- dialogs ---
  let viewing: { title: string; text: string } | null = $state(null);
  let exportOpen = $state(false);
  let exportFormat: "json" | "nmsbase" = $state("json");
  let importing = $state(false);
  let importingBusy = $state(false);
  let importMode: "paste" | "file" = $state("file");
  let importText = $state("");
  // Held outside reactivity on purpose: full base files can be tens of MB
  // and must never be rendered (that blew the dialog layout apart).
  // `hasImportFile` mirrors its presence for the template (reactively).
  let importFileText = "";
  let hasImportFile = $state(false);
  let importFileName: string | null = $state(null);
  let importFileInfo: { objects: number; kind: string } | { error: string } | null =
    $state(null);

  function resetImportDialog() {
    importing = false;
    importText = "";
    importFileText = "";
    hasImportFile = false;
    importFileName = null;
    importFileInfo = null;
    importMode = "file";
  }

  function summarizeImport(raw: string): { objects: number; kind: string } | { error: string } {
    let t = raw.trim();
    while (t.startsWith(",")) t = t.slice(1).trimStart();
    const tryParse = (s: string): unknown | undefined => {
      try {
        return JSON.parse(s);
      } catch {
        return undefined;
      }
    };
    let v = tryParse(t);
    if (v === undefined && t.startsWith("{")) v = tryParse(`[${t}]`);
    if (v === undefined) return { error: "Couldn't parse it — Inject will validate" };
    if (Array.isArray(v)) {
      if (!v.length) return { error: "Empty list" };
      const first = v[0] as Record<string, unknown>;
      if (first && typeof first === "object" && "ObjectID" in first)
        return { objects: v.length, kind: "Objects array" };
      if (first && typeof first === "object" && "Objects" in first)
        return { objects: (first.Objects as unknown[]).length, kind: "Full base" };
      return { objects: v.length, kind: "JSON list" };
    }
    if (v && typeof v === "object") {
      const o = v as Record<string, unknown>;
      if ("Objects" in o) return { objects: (o.Objects as unknown[]).length, kind: "Full base" };
      if ("ObjectID" in o) return { objects: 1, kind: "Single object" };
    }
    return { error: "Unrecognized format" };
  }
  let restoring = $state(false);
  let backups: BackupInfo[] = $state([]);
  let selectedBackup: string | null = $state(null);
  let recompressMode: "output" | "overwrite" | null = $state(null);
  let settingsOpen = $state(false);
  async function copyText(t: string): Promise<boolean> {
    try {
      await navigator.clipboard.writeText(t);
      return true;
    } catch {
      return false;
    }
  }

  // --- derived list ---
  let shown = $derived.by(() => {
    let list = [...bases];
    if (filter === "Corvettes") list = list.filter((b) => b.base_type === "PlayerShipBase");
    else if (filter === "Planetary")
      list = list.filter((b) => b.base_type === "HomePlanetBase" || b.base_type === "ExternalPlanetBase");
    const q = search.trim().toLowerCase();
    if (q)
      list = list.filter((b) =>
        `${b.display_name} ${b.name} ${b.base_type}`.toLowerCase().includes(q),
      );
    const dirMul = sortDir === "desc" ? -1 : 1;
    const byName = (a: BaseSummary, b: BaseSummary) =>
      dirMul * a.display_name.localeCompare(b.display_name);
    if (sortKey === "objects")
      list.sort((a, b) =>
        sortDir === "desc" ? b.objects - a.objects : a.objects - b.objects,
      );
    else if (sortKey === "type")
      list.sort(
        (a, b) =>
          dirMul * a.base_type.localeCompare(b.base_type) ||
          a.display_name.localeCompare(b.display_name),
      );
    else list.sort(byName);
    return list;
  });

  function setSort(key: BasesSortKey) {
    if (sortKey === key) {
      sortDir = sortDir === "asc" ? "desc" : "asc";
    } else {
      sortKey = key;
      sortDir = key === "objects" ? "desc" : "asc";
    }
    saveBasesSort({ key: sortKey, dir: sortDir });
  }
  $effect(() => {
    // Never auto-select: a stale selection (filtered out, new save loaded)
    // simply clears. The user always picks.
    if (selectedBase !== null && !shown.some((b) => b.idx === selectedBase))
      selectedBase = null;
  });

  function selectedBaseObj(): BaseSummary | null {
    return bases.find((b) => b.idx === selectedBase) ?? null;
  }

  let sel = $derived(selectedBaseObj());

  function typeBadgeVariant(t: string): "default" | "secondary" | "outline" {
    if (t === "PlayerShipBase") return "default";
    if (t === "HomePlanetBase" || t === "ExternalPlanetBase") return "secondary";
    return "outline";
  }
  function typeBadgeClass(t: string): string {
    if (t === "HomePlanetBase" || t === "ExternalPlanetBase")
      return "border-emerald-500/40 text-emerald-400";
    if (t === "FreighterBase") return "border-amber-500/40 text-amber-400";
    return "";
  }
  function shortType(t: string): string {
    if (t === "PlayerShipBase") return "Corvette";
    if (t === "HomePlanetBase" || t === "ExternalPlanetBase") return "Planetary";
    if (t === "FreighterBase") return "Freighter";
    if (t === "PlayerSpaceBase") return "Space";
    return t;
  }

  // --- actions ---
  async function refreshSaves() {
    if (!saveDir) return;
    try {
      saveFiles = await api.listSaveFiles(saveDir);
      if (saveFiles.length && !saveFiles.some((f) => f.name === selectedSave)) {
        selectedSave = saveFiles[0].name;
      }
      status = `${saveFiles.length} save(s) in ${saveDir}`;
    } catch (e) {
      toastErr(`List saves failed: ${e}`);
    }
  }

  async function detectDir() {
    // 1. remembered manual location wins (persisted in settings store)
    try {
      const manual = await loadSaveDirOverride();
      if (manual) {
        const files = await api.listSaveFiles(manual).catch(() => []);
        if (files.length) {
          saveDir = manual;
          saveDirManual = true;
          await refreshSaves();
          status = "Ready. Select a save, then press Load.";
          return;
        }
        // maybe they picked the parent NMS folder — drill into first child with saves
        const kids = await api.listSaveSubdirs(manual).catch(() => []);
        for (const kid of kids) {
          const kf = await api.listSaveFiles(kid).catch(() => []);
          if (kf.length) {
            saveDir = kid;
            saveDirManual = true;
            await refreshSaves();
            toastOk(`Using saves found in ${kid.split("/").slice(-1)}`);
            return;
          }
        }
        toastErr(`Remembered save folder has no saves: ${manual} — pick again or reset to auto`);
      }
    } catch (e) {
      toastErr(`Saved location failed: ${e}`);
    }
    // 2. platform autodetect (Proton/Steam/GOG/macOS — see README)
    try {
      saveDir = await api.findSaveDir(null);
      saveDirManual = false;
      if (!saveDir) {
        const dirs = await api.findSaveDirs();
        status = dirs.length
          ? "No save with .hg found"
          : "No save folder detected — choose it manually";
        return;
      }
      await refreshSaves();
      if (selectedSave) status = "Ready. Select a save, then press Load.";
    } catch (e) {
      toastErr(`Autodetect failed: ${e}`);
    }
  }

  async function doChangeDir() {
    const picked = await open({ directory: true, title: "Select save folder (st_… or DefaultUser)" });
    if (typeof picked === "string" && picked) {
      let dir = picked;
      // parent NMS folder picked? drill into first child that has saves
      const direct = await api.listSaveFiles(dir).catch(() => []);
      if (!direct.length) {
        const kids = await api.listSaveSubdirs(dir).catch(() => []);
        for (const kid of kids) {
          const kf = await api.listSaveFiles(kid).catch(() => []);
          if (kf.length) {
            dir = kid;
            toastOk(`Using saves found in ${kid.split("/").slice(-1)}`);
            break;
          }
        }
      }
      saveDir = dir;
      saveDirManual = true;
      try {
        await saveSaveDirOverride(dir);
      } catch (e) {
        toastErr(`Could not remember location: ${e}`);
      }
      selectedSave = null;
      bases = [];
      selectedBase = null;
      counts = null;
      await refreshSaves();
      if (!saveFiles.length) toastErr("No save*.hg files in that folder — try the st_… folder itself");
    }
  }

  async function resetSaveDir() {
    try {
      await clearSaveDirOverride();
    } catch {}
    saveDir = null;
    saveDirManual = false;
    selectedSave = null;
    bases = [];
    selectedBase = null;
    counts = null;
    await detectDir();
  }

  async function doLoad() {
    if (!saveDir || !selectedSave) {
      toastErr("No save selected");
      return;
    }
    loading = true;
    status = `Decompressing ${selectedSave}… (lz4 + mapping)`;
    try {
      const res = await api.decompressSave(saveDir, selectedSave);
      bases = res.bases;
      counts = res.counts;
      selectedBase = null;
      toastOk(`Loaded ${selectedSave}: ${bases.length} bases · backup ${res.backup_path.split("/").pop()}`);
    } catch (e) {
      toastErr(`Load failed: ${e}`);
    } finally {
      loading = false;
    }
  }

  function safeFileStem(name: string, ext: string): string {
    const stem = name.replace(/[^\w\- ]+/g, "").trim().replace(/ /g, "_") || "base";
    return `${stem}.${ext}`;
  }

  function openExportDialog() {
    if (selectedBaseObj()) {
      exportFormat = "json";
      exportOpen = true;
    } else {
      toastErr("Select a base first");
    }
  }

  let copying = $state(false);
  async function doExportCopy() {
    const b = selectedBaseObj();
    if (!b) return toastErr("Select a base first");
    copying = true;
    try {
      const text =
        exportFormat === "nmsbase" ? await api.getNmsbaseText(b.idx) : await api.getBaseJson(b.idx);
      if (await copyText(text)) {
        toastOk(
          `'${b.display_name}' copied as ${exportFormat === "nmsbase" ? "NMSBASE — paste after ^BASE_FLAG" : "JSON — paste in Base Builder"}`,
        );
        exportOpen = false;
      } else {
        toastErr("Clipboard copy failed");
      }
    } catch (e) {
      toastErr(`Copy failed: ${e}`);
    } finally {
      copying = false;
    }
  }

  async function doExportConfirm() {
    const b = selectedBaseObj();
    if (!b) return toastErr("Select a base first");
    exportOpen = false;
    if (exportFormat === "nmsbase") {
      await doExportNmsbase(b);
    } else {
      await doExportJson(b);
    }
  }

  async function doExportJson(b: BaseSummary) {
    try {
      const dest = await save({
        title: `Export '${b.display_name}' as JSON`,
        defaultPath: safeFileStem(b.display_name, "json"),
        filters: [
          { name: "JSON", extensions: ["json"] },
          { name: "All", extensions: ["*"] },
        ],
      });
      if (!dest) return;
      const res = await api.exportBase(b.idx, dest);
      const ok = await copyText(res.content);
      toastOk(
        `Exported '${b.display_name}' → ${res.path} · ${ok ? "copied — paste in Base Builder: Import base from NMS" : "clipboard copy failed"}`,
      );
    } catch (e) {
      toastErr(`Export failed: ${e}`);
    }
  }

  async function doExportNmsbase(b?: BaseSummary) {
    b ??= selectedBaseObj() ?? undefined;
    if (!b) return toastErr("Select a base first");
    try {
      const dest = await save({
        title: `Export '${b.display_name}' as NMSBASE`,
        defaultPath: safeFileStem(b.display_name, "nmsbase"),
        filters: [{ name: "NMSBASE", extensions: ["nmsbase", "json", "txt"] }],
      });
      if (!dest) return;
      const res = await api.exportNmsbase(b.idx, dest);
      const ok = await copyText(res.content);
      toastOk(`NMSBASE '${b.display_name}' → ${res.path} · ${ok ? "copied — paste after ^BASE_FLAG" : "file only"}`);
    } catch (e) {
      toastErr(`NMSBASE export failed: ${e}`);
    }
  }

  async function doView() {
    const b = selectedBaseObj();
    if (!b) return toastErr("Select a base first");
    try {
      const text = await api.getBaseJson(b.idx);
      viewing = { title: `${b.display_name} — slot ${b.idx}`, text };
    } catch (e) {
      toastErr(`View failed: ${e}`);
    }
  }

  function openImportDialog() {
    const b = selectedBaseObj();
    if (!b) {
      toastErr("Select target base to replace first");
      return;
    }
    resetImportDialog();
    importing = true;
  }

  async function doPickImportFile() {
    const picked = await open({
      title: "Import — pick JSON / NMSBASE / Objects",
      filters: [
        { name: "JSON / NMSBASE", extensions: ["json", "nmsbase", "txt"] },
        { name: "All", extensions: ["*"] },
      ],
    });
    if (typeof picked === "string" && picked) {
      try {
        importFileText = await api.readSaveTextFile(picked);
        hasImportFile = true;
        importFileName = picked.split("/").slice(-1)[0] ?? picked;
        importFileInfo = summarizeImport(importFileText);
        importText = "";
      } catch (e) {
        importFileText = "";
        hasImportFile = false;
        importFileName = null;
        importFileInfo = { error: `Read file failed: ${e}` };
      }
    }
  }

  async function doImport() {
    const b = selectedBaseObj();
    if (!b) return toastErr("Select target base to replace first");
    const payload = importMode === "file" ? importFileText : importText;
    if (!payload.trim()) {
      return toastErr(importMode === "file" ? "Pick a file first" : "Paste JSON first");
    }
    importingBusy = true;
    try {
      const res = await api.importBase(b.idx, payload);
      resetImportDialog();
      bases = await api.listBases(null);
      const c = { ship: 0, planet: 0, freighter: 0, space: 0, total_objs: 0 };
      for (const x of bases) {
        c.total_objs += x.objects;
        if (x.base_type === "PlayerShipBase") c.ship++;
        else if (x.base_type === "HomePlanetBase" || x.base_type === "ExternalPlanetBase") c.planet++;
        else if (x.base_type === "FreighterBase") c.freighter++;
        else if (x.base_type === "PlayerSpaceBase") c.space++;
      }
      counts = c;
      toastOk(
        `Injected ${res.objects} objects into '${b.display_name}' · original backed up — now Recompress to write .hg`,
      );
    } catch (e) {
      toastErr(`Inject failed: ${e}`);
    } finally {
      importingBusy = false;
    }
  }

  async function doRecompress() {
    if (!recompressMode || !selectedSave) return;
    const mode = recompressMode;
    recompressMode = null;
    try {
      status = "Recompressing…";
      const out = await api.recompressSave(mode);
      toastOk(`Recompressed → ${out}`);
    } catch (e) {
      toastErr(`Recompress failed: ${e}`);
    }
  }

  async function doBackup() {
    if (!saveDir) return toastErr("No save dir");
    try {
      const paths = await api.backupSaves(saveDir);
      toastOk(`Backed up ${paths.length} save file(s)`);
    } catch (e) {
      toastErr(`Backup failed: ${e}`);
    }
  }

  async function openRestore() {
    const stem = selectedSave?.replace(/\.hg$/, "") ?? null;
    try {
      backups = await api.listBackups(stem);
    } catch {
      backups = await api.listBackups(null);
    }
    if (!backups.length) return toastErr("No backups found");
    selectedBackup = backups[0].path;
    restoring = true;
  }

  async function doRestore() {
    if (!selectedBackup || !saveDir || !selectedSave) return;
    try {
      await api.restoreSave(selectedBackup, saveDir, selectedSave);
      restoring = false;
      bases = [];
      selectedBase = null;
      counts = null;
      await refreshSaves();
      toastOk(`Restored '${selectedSave}' — press Load to inspect`);
    } catch (e) {
      toastErr(`Restore failed: ${e}`);
    }
  }

  function onKey(e: KeyboardEvent) {
    const t = e.target as HTMLElement;
    if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA")) return;
    if (viewing || importing || restoring || recompressMode || settingsOpen || exportOpen) return;
    if (e.key === "e" || e.key === "E") openExportDialog();
    else if (e.key === "v") doView();
    else if (e.key === "i") openImportDialog();
    else if (e.key === "r") recompressMode = "output";
    else if (e.key === "c") filter = "Corvettes";
    else if (e.key === "p") filter = "Planetary";
    else if (e.key === "b") filter = "Both";
  }

  onMount(() => {
    (async () => {
      try {
        const s = await loadBasesSort();
        sortKey = s.key;
        sortDir = s.dir;
      } catch {}
    })();
    detectDir();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
</script>

<div class="flex min-h-0 flex-1 flex-col overflow-hidden bg-background text-foreground">
  <!-- top action bar -->
  <div class="flex min-h-12 shrink-0 flex-wrap items-center gap-1.5 border-b border-border bg-card px-3 py-1.5">
    <button
      class="flex min-w-0 items-center gap-1.5 rounded-md border border-border bg-background px-2 py-1 text-xs text-muted-foreground hover:bg-muted hover:text-foreground"
      onclick={doChangeDir}
      title={saveDir ? `${saveDir}${saveDirManual ? " (manual — click to change)" : " (auto-detected — click to override)"}` : "No save dir — click to choose"}
    >
      <FolderOpen class="size-3.5 shrink-0" />
      <span class="max-w-44 truncate font-mono">
        {saveDir ? saveDir.split("/").slice(-1)[0] : "No save dir"}
      </span>
      {#if saveDirManual}
        <Badge variant="default" class="h-4 px-1 text-[10px]">manual</Badge>
      {/if}
    </button>
    {#if saveDirManual}
      <Button
        variant="ghost"
        size="icon-sm"
        onclick={resetSaveDir}
        title="Forget manual folder, go back to auto-detect"
      >
        <X class="size-3.5" />
      </Button>
    {/if}
    {#if saveFiles.length > 0}
      <Select.Root
        type="single"
        value={selectedSave ?? ""}
        onValueChange={(v: string) => v && (selectedSave = v)}
      >
        <Select.Trigger class="h-7 w-36 font-mono text-xs">
          <Select.Value placeholder="Save…" />
        </Select.Trigger>
        <Select.Content>
          {#each saveFiles as f}
            <Select.Item value={f.name}>
              {f.name} · {f.size_display}
            </Select.Item>
          {/each}
        </Select.Content>
      </Select.Root>
      <Button size="sm" onclick={doLoad} disabled={loading || !selectedSave}>
        {#if loading}<LoaderCircle class="size-3.5 animate-spin" />Loading…{:else}<Download class="size-3.5" />Load{/if}
      </Button>
    {/if}
    <div class="mx-1 h-6 w-px bg-border"></div>
    <Button size="sm" variant="outline" onclick={doBackup} title="Back up all save files">
      <Save class="size-3.5" />Backup
    </Button>
    <Button size="sm" variant="outline" onclick={openRestore} title="Restore from backup…">
      <HardDriveDownload class="size-3.5" />Restore…
    </Button>
    <div class="mx-1 h-6 w-px bg-border"></div>
    <Button
      size="sm"
      variant="destructive"
      onclick={() => (recompressMode = "overwrite")}
      disabled={bases.length === 0}
    >
      Overwrite LIVE
    </Button>
    <Button
      size="sm"
      variant="outline"
      onclick={() => (recompressMode = "output")}
      disabled={bases.length === 0}
      title="Shortcut [r]"
    >
      Recompress
    </Button>
    <div class="ml-auto flex items-center gap-1.5">
      {#if counts}
        <span class="hidden text-xs text-muted-foreground xl:inline"
          >{counts.ship} ship · {counts.planet} planet · {counts.total_objs.toLocaleString()} objs</span
        >
      {/if}
      <Button variant="ghost" size="icon-sm" onclick={() => (settingsOpen = true)} title="Bases settings">
        <Settings class="size-4" />
      </Button>
    </div>
  </div>

  <DataList
    columns={[
      { id: "type", label: "Type", sortable: true },
      { id: "name", label: "Base", sortable: true },
      { id: "objects", label: "Objects", sortable: true, align: "right" },
      { id: "actions", label: "Actions", align: "right" },
    ]}
    gridTemplate="grid-cols-[auto_minmax(0,1fr)_auto_auto]"
    items={shown}
    keyOf={(b) => b.idx}
    isSelected={(b) => b.idx === selectedBase}
    sortKey={sortKey}
    sortDir={sortDir}
    onSort={(id) => setSort(id as "type" | "name" | "objects")}
    onSelect={(b) => (selectedBase = b.idx)}
    onActivate={() => openExportDialog()}
    onBackgroundClear={() => (selectedBase = null)}
  >
    {#snippet row(b)}
      <div>
        <Badge variant={typeBadgeVariant(b.base_type)} class={typeBadgeClass(b.base_type)}>
          {shortType(b.base_type)}
        </Badge>
      </div>
      <div class="min-w-0">
        <div class="truncate font-medium" title={b.display_name}>{b.display_name}</div>
        {#if b.name && b.name !== b.display_name}
          <div class="truncate font-mono text-[11px] text-muted-foreground" title={b.name}>
            {b.name}
          </div>
        {/if}
      </div>
      <div class="text-right font-mono text-xs">{b.objects.toLocaleString()}</div>
      <div class="flex justify-end">
        <Button
          variant="outline"
          size="xs"
          onclick={(e) => {
            e.stopPropagation();
            selectedBase = b.idx;
            openExportDialog();
          }}>Export</Button
        >
      </div>
    {/snippet}
    {#snippet empty()}
{#if bases.length === 0}
        <div class="p-12 text-center">
          <div class="mx-auto max-w-sm space-y-2">
            <div class="text-sm font-medium">
              {loading ? "Decompressing save…" : "No save loaded"}
            </div>
            <div class="text-xs text-muted-foreground">
              {#if loading}
                Reading LZ4 blocks and deobfuscating keys — this takes a few seconds.
              {:else if !saveDir}
                No save directory found. Point the app at your <span class="font-mono">st_…</span> folder.
              {:else}
                Pick a save above and press Load to list its bases.
              {/if}
            </div>
            {#if !loading && !saveDir}
              <Button size="sm" class="mt-2" onclick={doChangeDir}>
                <FolderOpen class="size-3.5" />Choose save folder…
              </Button>
            {:else if !loading && saveDir}
              <div class="mt-2 flex justify-center gap-2">
                <Button size="sm" onclick={doLoad} disabled={!selectedSave}>
                  <Download class="size-3.5" />Load {selectedSave ?? "save"}
                </Button>
                <Button size="sm" variant="outline" onclick={doChangeDir}>
                  <FolderOpen class="size-3.5" />Choose folder…
                </Button>
              </div>
            {/if}
          </div>
        </div>
      {:else if shown.length === 0}
        <div class="p-12 text-center">
          <div class="mx-auto max-w-sm space-y-2">
            <div class="text-sm font-medium">No matches</div>
            <div class="text-xs text-muted-foreground">
              Nothing matches "{search}" in this filter. Try clearing the search or switching to Both.
            </div>
            <Button
              size="sm"
              variant="outline"
              class="mt-2"
              onclick={() => {
                search = "";
                filter = "Both";
              }}
            >
              Clear search & filter
            </Button>
          </div>
        </div>
      {/if}
    {/snippet}
  </DataList>

  {#if sel}
    <div class="flex min-h-9 shrink-0 flex-wrap items-center gap-2 border-t border-border bg-muted px-3 py-1 text-xs">
      <span class="truncate font-medium">{sel.display_name}</span>
      <Badge variant={typeBadgeVariant(sel.base_type)} class={typeBadgeClass(sel.base_type)}>
        {shortType(sel.base_type)}
      </Badge>
      <span class="font-mono whitespace-nowrap text-muted-foreground"
        >slot {sel.idx} · {sel.objects.toLocaleString()} objs</span
      >
      <div class="ml-auto flex items-center gap-1.5">
        <Button size="xs" onclick={openExportDialog} title="Shortcut [e]">
          <Upload class="size-3" />Export…
        </Button>
        <Button size="xs" variant="outline" onclick={doView} title="Shortcut [v]">
          <Eye class="size-3" />View
        </Button>
        <Button size="xs" variant="outline" onclick={openImportDialog} title="Shortcut [i]">
          <Download class="size-3" />Import…
        </Button>
        <Button variant="ghost" size="xs" onclick={() => (selectedBase = null)}>Clear</Button>
      </div>
    </div>
  {/if}

  <div class="flex h-10 shrink-0 items-center gap-2 border-t border-border bg-muted/30 px-3">
    <Select.Root
      type="single"
      value={filter}
      onValueChange={(v: string) => v && (filter = v as typeof filter)}
    >
      <Select.Trigger class="h-7 w-32 bg-background text-xs">
        <Select.Value placeholder="Type" />
      </Select.Trigger>
      <Select.Content>
        <Select.Item value="Corvettes">Corvettes</Select.Item>
        <Select.Item value="Planetary">Planetary</Select.Item>
        <Select.Item value="Both">Both</Select.Item>
      </Select.Content>
    </Select.Root>
    <div class="relative max-w-sm flex-1">
      <Search class="pointer-events-none absolute top-1/2 left-2 size-3.5 -translate-y-1/2 text-muted-foreground" />
      <Input bind:value={search} placeholder="Search bases…" class="h-7 pl-7 text-xs" />
    </div>
    <span class="ml-auto hidden text-xs text-muted-foreground md:inline">{shown.length} shown</span>
  </div>
  <div class="shrink-0 truncate border-t border-border px-3 py-1 font-mono text-[11px] text-muted-foreground">
    {status}
  </div>

  <!-- toasts -->
  <div class="pointer-events-none fixed right-3 bottom-8 z-50 flex w-100 max-w-[calc(100vw-1.5rem)] flex-col gap-2">
    {#each toasts as t}
      <div
        class="pointer-events-auto flex items-start gap-2 rounded-lg border bg-card p-2.5 text-xs shadow-lg {t.kind === 'success'
          ? 'border-emerald-500/40'
          : t.kind === 'error'
            ? 'border-destructive/50'
            : 'border-border'}"
      >
        {#if t.kind === "success"}<CircleCheck class="mt-0.5 size-4 shrink-0 text-emerald-400" />
        {:else if t.kind === "error"}<CircleAlert class="mt-0.5 size-4 shrink-0 text-destructive" />
        {:else}<Info class="mt-0.5 size-4 shrink-0 text-muted-foreground" />{/if}
        <span class="wrap-break-word">{t.msg}</span>
      </div>
    {/each}
  </div>

  <!-- export format dialog -->
  <Dialog.Root bind:open={exportOpen}>
    <Dialog.Content class="max-w-md">
      <Dialog.Header>
        <Dialog.Title>Export '{selectedBaseObj()?.display_name ?? ""}'</Dialog.Title>
        <Dialog.Description>Choose a format — you'll pick where to save it next.</Dialog.Description>
      </Dialog.Header>
      <div class="flex flex-col gap-2 py-1">
        <button
          class="rounded-lg border p-3 text-left transition {exportFormat === 'json'
            ? 'border-primary bg-primary/10'
            : 'border-border hover:bg-muted'}"
          onclick={() => (exportFormat = "json")}
        >
          <div class="flex items-center gap-2 text-sm font-medium">
            <FileBraces class="size-4 text-primary" />Full JSON <Badge variant="default">.json</Badge>
          </div>
          <p class="mt-1 text-xs text-muted-foreground">
            Complete base data. Paste into Base Builder via <span class="font-medium">Import base from NMS</span>.
          </p>
        </button>
        <button
          class="rounded-lg border p-3 text-left transition {exportFormat === 'nmsbase'
            ? 'border-primary bg-primary/10'
            : 'border-border hover:bg-muted'}"
          onclick={() => (exportFormat = "nmsbase")}
        >
          <div class="flex items-center gap-2 text-sm font-medium">
            <FileBracesCorner class="size-4 text-amber-400" />NMSBASE <Badge variant="outline">.nmsbase</Badge>
          </div>
          <p class="mt-1 text-xs text-muted-foreground">
            Objects only. Paste into NomNom / NMSSE after the <span class="font-mono">^BASE_FLAG</span> entry.
          </p>
        </button>
      </div>
      <Dialog.Footer>
        <Button variant="outline" onclick={() => (exportOpen = false)}>Cancel</Button>
        <Button variant="outline" onclick={doExportCopy} disabled={copying}>
          {#if copying}<LoaderCircle class="size-3.5 animate-spin" />Copying…{:else}<Copy class="size-3.5" />Copy{/if}
        </Button>
        <Button onclick={doExportConfirm}>
          <Upload class="size-3.5" />Save…
        </Button>
      </Dialog.Footer>
    </Dialog.Content>
  </Dialog.Root>

  <!-- view dialog -->
  <Dialog.Root open={viewing !== null} onOpenChange={(o: boolean) => !o && (viewing = null)}>
    <Dialog.Content class="max-h-[85vh] max-w-3xl overflow-hidden">
      <Dialog.Header>
        <Dialog.Title>{viewing?.title ?? ""}</Dialog.Title>
        <Dialog.Description>Full base JSON as stored in the save.</Dialog.Description>
      </Dialog.Header>
      <pre class="max-h-[55vh] overflow-auto rounded-md border border-border bg-background p-3 font-mono text-[11px] break-all whitespace-pre-wrap">{viewing?.text ?? ""}</pre>
      <Dialog.Footer>
        <Button
          variant="outline"
          onclick={() => (viewing = null)}
        >
          Close
        </Button>
        <Button
          onclick={async () => {
            if (await copyText(viewing?.text ?? "")) toastOk("Base JSON copied to clipboard");
            viewing = null;
          }}
        >
          <Copy class="size-3.5" />Copy & close
        </Button>
      </Dialog.Footer>
    </Dialog.Content>
  </Dialog.Root>

  <!-- import dialog -->
  <Dialog.Root
    bind:open={importing}
    onOpenChange={(o: boolean) => {
      if (!o) resetImportDialog();
    }}
  >
    <Dialog.Content class="max-w-md overflow-hidden">
      <Dialog.Header>
        <Dialog.Title>Import into '{selectedBaseObj()?.display_name ?? ""}' (slot {selectedBase})</Dialog.Title>
        <Dialog.Description>
          Replaces the base's objects. The original is backed up automatically.
        </Dialog.Description>
      </Dialog.Header>
      <div class="grid grid-cols-2 gap-1 rounded-lg border border-border bg-background p-1">
        <button
          class="rounded-md px-1 py-1 text-xs font-medium transition {importMode === 'file'
            ? 'bg-primary text-primary-foreground'
            : 'text-muted-foreground hover:bg-muted hover:text-foreground'}"
          onclick={() => (importMode = "file")}
        >
          From file
        </button>
        <button
          class="rounded-md px-1 py-1 text-xs font-medium transition {importMode === 'paste'
            ? 'bg-primary text-primary-foreground'
            : 'text-muted-foreground hover:bg-muted hover:text-foreground'}"
          onclick={() => (importMode = "paste")}
        >
          Paste JSON
        </button>
      </div>
      {#if importMode === "paste"}
        <Textarea
          bind:value={importText}
          rows={6}
          class="max-h-56 font-mono text-xs"
          placeholder="Paste base JSON, objects array, or .nmsbase text…"
        />
      {:else}
        <div class="flex flex-col gap-2">
          <Button variant="outline" onclick={doPickImportFile}>
            <FolderOpen class="size-3.5" />{importFileName ? "Pick a different file…" : "Pick file…"}</Button
          >
          {#if importFileName}
            <div class="rounded-md border border-border bg-background px-2.5 py-2">
              <div class="truncate font-mono text-xs" title={importFileName}>{importFileName}</div>
              {#if importFileInfo && "objects" in importFileInfo}
                <div class="mt-1 flex gap-1.5">
                  <Badge variant="default">{importFileInfo.objects.toLocaleString()} objects</Badge>
                  <Badge variant="outline">{importFileInfo.kind}</Badge>
                </div>
              {:else if importFileInfo && "error" in importFileInfo}
                <div class="mt-1 text-xs text-amber-400">{importFileInfo.error}</div>
              {/if}
            </div>
          {:else}
            <p class="text-xs text-muted-foreground">
              Full base JSON, objects-only arrays, and .nmsbase files are all accepted.
            </p>
          {/if}
        </div>
      {/if}
      <Dialog.Footer>
        <Button variant="outline" onclick={resetImportDialog}>Cancel</Button>
        <Button
          onclick={doImport}
          disabled={importingBusy || (importMode === "file" ? !hasImportFile : !importText.trim())}
        >
          {#if importingBusy}<LoaderCircle class="size-3.5 animate-spin" />Injecting…{:else}Inject{/if}
        </Button>
      </Dialog.Footer>
    </Dialog.Content>
  </Dialog.Root>

  <!-- restore dialog -->
  <Dialog.Root bind:open={restoring}>
    <Dialog.Content class="max-h-[85vh] max-w-2xl overflow-hidden">
      <Dialog.Header>
        <Dialog.Title>Restore '{selectedSave}'</Dialog.Title>
        <Dialog.Description>
          The current live file is backed up again before restoring. Close NMS first!
        </Dialog.Description>
      </Dialog.Header>
      <div class="max-h-[50vh] overflow-auto rounded-md border border-border">
        <Table.Root>
          <Table.Header>
            <Table.Row>
              <Table.Head>Backup file</Table.Head>
              <Table.Head class="w-20">Size</Table.Head>
              <Table.Head class="w-36">Modified</Table.Head>
            </Table.Row>
          </Table.Header>
          <Table.Body>
            {#each backups as bk}
              <Table.Row
                class="cursor-pointer {bk.path === selectedBackup ? 'bg-primary/10 hover:bg-primary/15' : ''}"
                onclick={() => (selectedBackup = bk.path)}
              >
                <Table.Cell class="font-mono text-xs">{bk.name}</Table.Cell>
                <Table.Cell>{bk.size_display}</Table.Cell>
                <Table.Cell class="font-mono text-xs">{bk.modified}</Table.Cell>
              </Table.Row>
            {/each}
          </Table.Body>
        </Table.Root>
      </div>
      <Dialog.Footer>
        <Button variant="outline" onclick={() => (restoring = false)}>Cancel</Button>
        <Button variant="destructive" onclick={doRestore}>Restore selected</Button>
      </Dialog.Footer>
    </Dialog.Content>
  </Dialog.Root>

  <!-- recompress confirm -->
  <Dialog.Root open={recompressMode !== null} onOpenChange={(o: boolean) => !o && (recompressMode = null)}>
    <Dialog.Content class="max-w-md">
      <Dialog.Header>
        <Dialog.Title>
          {recompressMode === "overwrite" ? "Overwrite live save?" : "Write recompressed save?"}
        </Dialog.Title>
        <Dialog.Description>
          {#if recompressMode === "overwrite"}
            Writes directly over <span class="font-mono">{selectedSave}</span>. A
            <span class="font-mono">*_before_recompress_*.hg</span> backup is made first. Make sure NMS is closed —
            Steam Cloud can revert the change.
          {:else}
            Writes a recompressed copy of <span class="font-mono">{selectedSave}</span> to the output folder,
            leaving the live save untouched.
          {/if}
        </Dialog.Description>
      </Dialog.Header>
      <Dialog.Footer>
        <Button variant="outline" onclick={() => (recompressMode = null)}>Cancel</Button>
        <Button variant={recompressMode === "overwrite" ? "destructive" : "default"} onclick={doRecompress}>
          {recompressMode === "overwrite" ? "Overwrite LIVE" : "Write to output/"}
        </Button>
      </Dialog.Footer>
    </Dialog.Content>
  </Dialog.Root>

  <!-- settings dialog -->
  <Dialog.Root bind:open={settingsOpen}>
    <Dialog.Content class="max-w-sm">
      <Dialog.Header>
        <Dialog.Title>Bases settings</Dialog.Title>
        <Dialog.Description>Where your No Man's Sky saves live.</Dialog.Description>
      </Dialog.Header>
      <div>
        <div class="mb-1 text-xs font-medium">Save location {saveDirManual ? "(manual)" : "(auto-detected)"}</div>
        <div class="rounded-md border border-border bg-background px-2 py-1.5 font-mono text-[11px] break-all">
          {saveDir ?? "Not detected yet"}
        </div>
        <div class="mt-1.5 flex gap-2">
          <Button size="sm" variant="outline" class="flex-1" onclick={doChangeDir}>
            <FolderOpen class="size-3.5" />Choose…
          </Button>
          {#if saveDirManual}
            <Button
              size="sm"
              variant="ghost"
              class="flex-1"
              onclick={() => {
                resetSaveDir();
              }}
            >
              Reset to auto
            </Button>
          {/if}
        </div>
      </div>
      <Separator />
      <Dialog.Footer>
        <Button onclick={() => (settingsOpen = false)}>Done</Button>
      </Dialog.Footer>
    </Dialog.Content>
  </Dialog.Root>
</div>
