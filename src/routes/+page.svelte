<script lang="ts">
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { api } from "$lib/api";
  import { loadConfigNative, saveConfigNative } from "$lib/config";
  import type { Mod, Profile, AppConfig } from "$lib/types";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import * as Select from "$lib/components/ui/select";
  import * as Dialog from "$lib/components/ui/dialog";
  import * as Table from "$lib/components/ui/table";
  import { LoaderCircle } from "lucide-svelte";
  import DataList from "$lib/components/data-list.svelte";
  import SortHeader from "$lib/components/sort-header.svelte";
  import { loadTableSort, saveTableSort, type SortDir } from "$lib/table-sort";

  let gameRoot: string | null = $state(null);
  let modsDir = $state("");
  let storeDir = $state("");
  let mods: Mod[] = $state([]);
  let profile: Profile | null = $state(null);
  let profiles: string[] = $state([]);
  let config: AppConfig | null = $state(null);
  let selectedIds: Set<string> = $state(new Set());
  let lastSelected: string | null = $state(null);
  let deployedMap: Record<string, string> = $state({});
  let foreignMap: Record<string, string> = $state({});
  let foreignSelected: Set<string> = $state(new Set());
  let globalDisabled = $state(false);
  let status = $state("Ready");
  let isAdding = $state(false);
  let isDragging = $state(false);
  let filter = $state("");
  let statusFilter: string = $state("All");
  type ModSortKey = "manual" | "status" | "name" | "size";
  const MOD_SORT_KEY = "mods_sort";
  let sortKey: ModSortKey = $state("manual");
  let sortDir: SortDir = $state("asc");

  function defaultDirFor(key: ModSortKey): SortDir {
    return key === "name" ? "asc" : "desc";
  }
  function setModSort(key: Exclude<ModSortKey, "manual">) {
    if (sortKey !== key) {
      sortKey = key;
      sortDir = defaultDirFor(key);
    } else if (sortDir === defaultDirFor(key)) {
      sortDir = sortDir === "asc" ? "desc" : "asc";
    } else {
      // third click: back to manual deploy order
      sortKey = "manual";
      sortDir = "asc";
    }
    saveTableSort(MOD_SORT_KEY, { key: sortKey, dir: sortDir });
  }
  let isSorted = $derived(sortKey !== "manual");
  let lastAddKey = "";
  let lastAddAt = 0;
  let showRemoveConfirm = $state(false);
  let removeConfirmCount = $state(0);

  function sizeStr(b: number) {
    if (b > 1024 * 1024) return `${(b / 1024 / 1024).toFixed(1)} MB`;
    if (b > 1024) return `${(b / 1024).toFixed(0)} KB`;
    return `${b} B`;
  }

  async function refreshDeployed() {
    if (!modsDir) return;
    try {
      const scan = await api.scanDeployed(modsDir);
      deployedMap = scan.managed;
      foreignMap = scan.foreign;
      const valid = new Set(
        [...foreignSelected].filter((n) => n in foreignMap),
      );
      if (valid.size !== foreignSelected.size) foreignSelected = valid;
    } catch {}
  }

  async function refreshGamePath() {
    try {
      config = await loadConfigNative();
    } catch {
      config = await api.loadConfig();
    }
    const found = await api.findNmsInstall(config.game_path);
    gameRoot = found;
    modsDir = found ? await api.getModsDir(found) : "";
    storeDir = await api.getStoreDir();
    if (modsDir) {
      try {
        globalDisabled = await api.globalDisableEnabled(modsDir);
        await refreshDeployed();
      } catch {}
    }
  }
  async function refreshProfiles() {
    profiles = await api.listProfiles();
    if (!profiles.length) {
      await api.createProfile("default");
      profiles = await api.listProfiles();
    }
    profile = await api.loadProfile(config?.active_profile || profiles[0]);
    await refreshMods();
  }
  async function refreshMods() {
    if (!profile) return;
    const all = await api.scanStore();
    const map = new Map(all.map((m) => [m.id, m]));
    const ordered: Mod[] = [];
    for (const id of profile.mod_order) {
      const m = map.get(id);
      if (m) ordered.push(m);
    }
    for (const m of all) if (!profile.mod_order.includes(m.id)) ordered.push(m);
    mods = ordered;
    const valid = new Set(
      [...selectedIds].filter((id) => mods.some((m) => m.id === id)),
    );
    if (valid.size !== selectedIds.size) selectedIds = valid;
    await refreshDeployed();
  }

  let lastClickId: string | null = null;
  let lastClickAt = 0;
  function handleSelect(id: string, e: MouseEvent) {
    const now = Date.now();
    const isDouble = id === lastClickId && now - lastClickAt < 350;
    lastClickId = id;
    lastClickAt = now;
    const isCtrl = e.ctrlKey || e.metaKey;
    const isShift = e.shiftKey;
    if (isShift && lastSelected) {
      const s = mods.findIndex((m) => m.id === lastSelected);
      const t = mods.findIndex((m) => m.id === id);
      if (s !== -1 && t !== -1) {
        const [a, b] = s < t ? [s, t] : [t, s];
        selectedIds = new Set(mods.slice(a, b + 1).map((m) => m.id));
      } else {
        selectedIds = new Set([id]);
        lastSelected = id;
      }
    } else if (isCtrl) {
      const n = new Set(selectedIds);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      selectedIds = n;
      lastSelected = n.has(id) ? id : n.size ? [...n].pop()! : null;
    } else {
      // plain click: if already solely selected, unselect (unless it's a double-click)
      if (selectedIds.has(id) && selectedIds.size === 1 && !isDouble) {
        selectedIds = new Set();
        lastSelected = null;
        return;
      }
      // double-click second click should not toggle selection away
      if (isDouble && selectedIds.has(id)) return;
      selectedIds = new Set([id]);
      lastSelected = id;
    }
  }
  function toggleMod(mod: Mod) {
    const cur = profile?.enabled[mod.id] ?? true;
    if (profile) {
      profile.enabled[mod.id] = !cur;
      api
        .saveProfile(profile)
        .then(() => refreshMods())
        .then(() => maybeAutoDeploy());
    }
  }
  function orderOf(mod: Mod): string {
    if (!profile) return "–";
    const i = profile.mod_order.indexOf(mod.id);
    return i >= 0 ? String(i + 1) : "–";
  }

  // --- rename dialog ---
  let renameTarget: Mod | null = $state(null);
  let renameValue = $state("");
  let renameInput: HTMLInputElement | null = $state(null);
  let renameBusy = $state(false);
  let renameUnchanged = $state(false);
  let renameError: string | null = $state(null);
  $effect(() => {
    if (renameTarget && renameInput) {
      renameInput.focus();
      renameInput.select();
    }
  });
  function openRenameDialog(mod: Mod) {
    renameValue = mod.display_name;
    renameUnchanged = false;
    renameError = null;
    renameTarget = mod;
  }
  async function commitRenameDialog() {
    const mod = renameTarget;
    if (!mod) {
      renameError = "No mod selected — reopen the dialog and try again.";
      return;
    }
    const next = (renameValue ?? "").trim();
    if (!next) {
      renameError = "Type a name first.";
      return;
    }
    const current = mods.find((m) => m.id === mod.id);
    if (!current) {
      renameError = "That mod is no longer in the list — reload and try again.";
      status = renameError;
      return;
    }
    if (next === current.display_name) {
      renameUnchanged = true;
      return;
    }
    renameUnchanged = false;
    renameError = null;
    renameBusy = true;
    try {
      const newId = await api.renameStoreMod(mod.id, next);
      if (profile) {
        profile.mod_order = profile.mod_order.map((x) =>
          x === mod.id ? newId : x,
        );
        if (mod.id in profile.enabled) {
          profile.enabled[newId] = profile.enabled[mod.id];
          delete profile.enabled[mod.id];
        }
        await api.saveProfile(profile);
      }
      if (lastSelected === mod.id) lastSelected = newId;
      selectedIds = new Set(
        [...selectedIds].map((x) => (x === mod.id ? newId : x)),
      );
      await refreshMods();
      status = `Renamed to ${newId}`;
      renameTarget = null;
      await maybeAutoDeploy();
    } catch (err: any) {
      const msg = `${err}`;
      status = msg;
      renameError = msg;
    } finally {
      renameBusy = false;
    }
  }
  function handleClear() {
    if (selectedIds.size) {
      selectedIds = new Set();
      lastSelected = null;
    }
  }

  async function handleReorder(
    fromId: string,
    toId: string,
    pos: "before" | "after" = "before",
  ) {
    if (!profile || fromId === toId) return;
    const isBlock = selectedIds.has(fromId) && selectedIds.size > 1;
    if (isBlock) {
      const blockSet = new Set(selectedIds);
      if (blockSet.has(toId)) return;
      const block = profile.mod_order.filter((id) => blockSet.has(id));
      const remaining = profile.mod_order.filter((id) => !blockSet.has(id));
      const idx = remaining.indexOf(toId);
      const ins =
        idx === -1 ? remaining.length : idx + (pos === "after" ? 1 : 0);
      profile.mod_order = [
        ...remaining.slice(0, ins),
        ...block,
        ...remaining.slice(ins),
      ];
      await api.saveProfile(profile);
      await refreshMods();
      status = `Moved ${block.length}`;
      await maybeAutoDeploy();
      return;
    }
    const from = profile.mod_order.indexOf(fromId);
    const to = profile.mod_order.indexOf(toId);
    if (from === -1 || to === -1) return;
    const [m] = profile.mod_order.splice(from, 1);
    const ins =
      from < to
        ? pos === "before"
          ? to - 1
          : to
        : pos === "before"
          ? to
          : to + 1;
    profile.mod_order.splice(
      Math.max(0, Math.min(ins, profile.mod_order.length)),
      0,
      m,
    );
    await api.saveProfile(profile);
    await refreshMods();
    selectedIds = new Set([fromId]);
    lastSelected = fromId;
    await maybeAutoDeploy();
  }

  async function handleAddPaths(paths: string[]) {
    if (!paths.length || isAdding) return;
    const key = [...paths].sort().join("|");
    const now = Date.now();
    if (key === lastAddKey && now - lastAddAt < 1500) return;
    lastAddKey = key;
    lastAddAt = now;
    isAdding = true;
    status = `Adding ${paths.length}…`;
    try {
      const res = await api.addMods(paths);
      if (res.imported.length) profile = await api.loadProfile(profile!.name);
      await refreshMods();
      await maybeAutoDeploy();
      status = res.imported.length
        ? `Added ${res.imported.length}`
        : `Skipped — ${res.skipped[0] ?? ""}`;
    } catch (e: any) {
      status = `${e}`;
    } finally {
      isAdding = false;
      setTimeout(() => (status = "Ready"), 2500);
    }
  }
  async function addModsFile() {
    const p = await open({
      multiple: true,
      filters: [
        { name: "NMS Mods", extensions: ["pak", "zip"] },
        { name: "All", extensions: ["*"] },
      ],
    });
    if (!p) return;
    await handleAddPaths(Array.isArray(p) ? (p as string[]) : [p as string]);
  }
  async function doImport(mv: boolean) {
    if (!modsDir) return;
    const r = await api.importMods(modsDir, mv);
    if (r.imported.length) profile = await api.loadProfile(profile!.name);
    await refreshMods();
    await maybeAutoDeploy();
    status = r.imported.length
      ? `Imported ${r.imported.length}`
      : "Nothing to import";
  }
  async function doDeploy() {
    if (!modsDir || !profile) return;
    const ids = profile.mod_order.filter(
      (id) => (profile!.enabled[id] ?? true) && mods.some((m) => m.id === id),
    );
    status = "Deploying…";
    const r = await api.deployMods(modsDir, ids, config?.deploy_mode || "auto");
    status = r.errors.length ? r.errors[0] : `Deployed ${r.deployed}`;
    await refreshMods();
  }
  async function maybeAutoDeploy() {
    if (!config?.auto_deploy || !modsDir || !profile) return;
    await doDeploy();
  }
  async function doPurge() {
    if (!modsDir) return;
    if (!confirm("Purge deployed mods?")) return;
    await api.deployMods(modsDir, [], config?.deploy_mode || "auto");
    status = `Purged`;
    await refreshMods();
  }
  async function removeSelected() {
    if (!profile || !selectedIds.size) return;
    for (const id of [...selectedIds]) {
      profile.mod_order = profile.mod_order.filter((x) => x !== id);
      delete profile.enabled[id];
      await api.removeStoreMod(id);
    }
    await api.saveProfile(profile);
    selectedIds = new Set();
    await refreshMods();
    await maybeAutoDeploy();
    showRemoveConfirm = false;
  }

  function openRemoveConfirm() {
    if (!selectedIds.size) return;
    removeConfirmCount = selectedIds.size;
    showRemoveConfirm = true;
  }

  // --- Foreign mods (in MODS but not managed by NMM) ---
  let foreignNames = $derived(Object.keys(foreignMap).sort((a, b) => a.localeCompare(b)));
  let foreignBusy = $state(false);
  function toggleForeign(name: string) {
    const n = new Set(foreignSelected);
    if (n.has(name)) n.delete(name);
    else n.add(name);
    foreignSelected = n;
  }
  async function importForeign(names: string[]) {
    if (!modsDir || !names.length || foreignBusy) return;
    foreignBusy = true;
    status = `Importing ${names.length}…`;
    try {
      const r = await api.importModsSelected(modsDir, names, false);
      if (r.imported.length && profile)
        profile = await api.loadProfile(profile.name);
      await refreshMods();
      await maybeAutoDeploy();
      status = r.imported.length
        ? `Imported ${r.imported.length}`
        : (`Skipped — ${r.skipped[0] ?? ""}`);
    } catch (e: any) {
      status = `${e}`;
    } finally {
      foreignBusy = false;
      setTimeout(() => (status = "Ready"), 2500);
    }
  }

  let filtered = $derived.by(() => {
    const list = mods.filter((m) => {
      const q = filter.toLowerCase();
      const matchesName = !q || m.display_name.toLowerCase().includes(q);
      const enabled = profile?.enabled[m.id] ?? true;
      const matchesStatus =
        statusFilter === "All" ||
        (statusFilter === "Enabled" && enabled) ||
        (statusFilter === "Disabled" && !enabled);
      return matchesName && matchesStatus;
    });
    // Display-only: deploy (profile) order is never touched by sorting.
    if (sortKey === "status") {
      const mul = sortDir === "desc" ? 1 : -1;
      list.sort(
        (a, b) =>
          mul *
          (((profile?.enabled[b.id] ?? true) ? 1 : 0) -
            ((profile?.enabled[a.id] ?? true) ? 1 : 0)),
      );
    } else if (sortKey === "name") {
      const mul = sortDir === "desc" ? -1 : 1;
      list.sort((a, b) => mul * a.display_name.localeCompare(b.display_name));
    } else if (sortKey === "size") {
      list.sort((a, b) =>
        sortDir === "desc"
          ? b.size_bytes - a.size_bytes
          : a.size_bytes - b.size_bytes,
      );
    }
    return list;
  });

  onMount(() => {
    let cleanup: (() => void) | undefined;
    (async () => {
      try {
        const s = await loadTableSort(
          MOD_SORT_KEY,
          ["manual", "status", "name", "size"] as const,
          { key: "manual", dir: "asc" },
        );
        sortKey = s.key;
        sortDir = s.dir;
      } catch {}
      await refreshGamePath();
      await refreshProfiles();
      if (!gameRoot) showStartupDialog = true;
      try {
        const { getCurrentWebview } = await import("@tauri-apps/api/webview");
        const un = await getCurrentWebview().onDragDropEvent((e) => {
          if (e.payload.type === "enter" || e.payload.type === "over")
            isDragging = true;
          else if (e.payload.type === "leave") isDragging = false;
          else if (e.payload.type === "drop") {
            isDragging = false;
            const p = (e.payload as any).paths as string[] | undefined;
            if (p?.length) handleAddPaths(p);
          }
        });
        cleanup = un;
      } catch {}
    })();
    const onKey = (e: KeyboardEvent) => {
      // Escape only deselects / closes dialogs. There are no destructive
      // keyboard shortcuts anywhere: deletion always goes through the
      // Remove button + confirmation dialog.
      if (e.key === "Escape") {
        if (showStartupDialog) showStartupDialog = false;
        else handleClear();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      if (cleanup) cleanup();
    };
  });
  let enabledCount = $derived(
    profile
      ? Object.values((profile as Profile).enabled).filter(Boolean).length
      : 0,
  );
  let showStartupDialog = $state(false);

  async function pickGamePathStartup() {
    const picked = await open({
      directory: true,
      title: "Select No Man's Sky folder",
    });
    if (typeof picked === "string" && picked) {
      if (!config)
        try {
          config = await loadConfigNative();
        } catch {
          config = await api.loadConfig();
        }
      config.game_path = picked;
      try {
        await saveConfigNative(config);
      } catch {
        await api.saveConfig(config);
      }
      await refreshGamePath();
      await refreshProfiles();
      if (gameRoot) showStartupDialog = false;
    }
  }
  async function retryAutoDetect() {
    if (!config)
      try {
        config = await loadConfigNative();
      } catch {
        config = await api.loadConfig();
      }
    config.game_path = null;
    try {
      await saveConfigNative(config);
    } catch {
      await api.saveConfig(config);
    }
    await refreshGamePath();
    await refreshProfiles();
    if (gameRoot) showStartupDialog = false;
    else status = "Still not found — please choose folder";
  }
</script>

<div class="flex flex-1 flex-col min-w-0 bg-background overflow-hidden">
  <div class="h-12 shrink-0 flex items-center gap-1 px-3 border-b bg-card/70 backdrop-blur supports-[backdrop-filter]:bg-card/70">
    <Button
      variant="default"
      size="sm"
      onclick={addModsFile}
      disabled={isAdding}><span>＋</span> Add Mod</Button
    >
    <Button variant="outline" size="sm" onclick={() => doImport(false)}
      >Import</Button
    >
    <div class="h-6 w-px bg-border mx-1"></div>
    <Button variant="default" size="sm" onclick={doDeploy}>Deploy</Button>
    <div class="ml-auto flex items-center gap-2">
      <span class="hidden sm:inline text-xs text-muted-foreground"
        >{enabledCount}/{mods.length} enabled</span
      >
      <Button
        variant="outline"
        size="icon-sm"
        onclick={async () => modsDir && (await api.openFolder(modsDir))}
        title="Open MODS">📁</Button
      >
    </div>
  </div>

  <DataList
    columns={[
      { id: "order", label: "Order", sortable: true },
      { id: "status", label: "Status", sortable: true },
      { id: "name", label: "Mod Name", sortable: true },
      { id: "size", label: "Size", sortable: true, align: "right" },
      { id: "actions", label: "Actions", align: "right" },
    ]}
    items={filtered}
    keyOf={(m) => m.id}
    isSelected={(m) => selectedIds.has(m.id)}
    sortKey={sortKey === "manual" ? "order" : sortKey}
    sortDir={sortKey === "manual" ? "asc" : sortDir}
    onSort={(id) => {
      if (id === "order") {
        sortKey = "manual";
        sortDir = "asc";
        saveTableSort(MOD_SORT_KEY, { key: sortKey, dir: sortDir });
      } else {
        setModSort(id as "status" | "name" | "size");
      }
    }}
    onSelect={(m, e) => handleSelect(m.id, e as unknown as MouseEvent)}
    onBackgroundClear={handleClear}
    isDraggable={() => !isSorted}
    onReorder={(from, to, pos) => handleReorder(String(from), String(to), pos)}
  >
    {#snippet row(mod, isSel)}
      {@const enabled = profile?.enabled[mod.id] ?? true}
      <Table.Cell class="font-mono text-xs text-muted-foreground">
        {orderOf(mod)}
      </Table.Cell>
      <Table.Cell>
        <button
          type="button"
          class="inline-flex cursor-pointer items-center rounded-full border px-2 py-0.5 text-[11px] font-medium transition {enabled
            ? 'bg-primary text-primary-foreground border-primary hover:bg-primary/80'
            : 'bg-muted text-muted-foreground hover:bg-muted/70'}"
          title={enabled ? "Click to disable" : "Click to enable"}
          onclick={(e) => {
            e.stopPropagation();
            toggleMod(mod);
          }}
        >
          {enabled ? "Enabled" : "Disabled"}
        </button>
      </Table.Cell>
      <Table.Cell class="max-w-md">
        <div
          class="truncate font-medium {enabled ? '' : 'opacity-60'}"
          role="button"
          tabindex="0"
          title={`${mod.display_name} — double-click to rename`}
          onclick={(e) => {
            e.stopPropagation();
            handleSelect(mod.id, e);
          }}
          onkeydown={(e) => {
            if (e.key === "Enter") {
              e.stopPropagation();
              openRenameDialog(mod);
            }
          }}
          ondblclick={(e) => {
            e.stopPropagation();
            openRenameDialog(mod);
          }}
        >
          {mod.display_name}
        </div>
      </Table.Cell>
      <Table.Cell class="font-mono text-xs text-muted-foreground">
        {sizeStr(mod.size_bytes)}
      </Table.Cell>
      <Table.Cell>
        <div class="flex justify-end gap-1.5">
          <Button
            variant="ghost"
            size="xs"
            onclick={(e) => {
              e.stopPropagation();
              openRenameDialog(mod);
            }}>Rename</Button
          >
          <Button
            variant="outline"
            size="xs"
            onclick={(e) => {
              e.stopPropagation();
              handleSelect(mod.id, new MouseEvent("click"));
              openRemoveConfirm();
            }}>Remove</Button
          >
        </div>
      </Table.Cell>
    {/snippet}
    {#snippet empty()}
      <div class="p-12 text-center">
        <div class="mx-auto max-w-sm space-y-2">
          <div class="text-sm font-medium">
            {mods.length === 0 ? "No mods yet" : "No matches"}
          </div>
          <div class="text-xs text-muted-foreground">
            {mods.length === 0
              ? "Add .pak or .zip, or import from MODS."
              : `No results for "${filter}"`}
          </div>
          {#if mods.length === 0}<Button
              variant="default"
              size="sm"
              class="mt-2"
              onclick={addModsFile}>Add mods</Button
            >{/if}
        </div>
      </div>
    {/snippet}
  </DataList>

  {#if foreignNames.length > 0}
    <div class="shrink-0 border-t bg-card">
      <div class="flex items-center gap-2 px-3 pt-2 pb-1">
        <span class="text-xs font-semibold"
          >Unmanaged in MODS ({foreignNames.length})</span
        >
        <span class="text-[11px] text-muted-foreground"
          >not deployed by NMM — select and import to manage</span
        >
        <div class="ml-auto flex items-center gap-1.5">
          <Button
            variant="outline"
            size="xs"
            disabled={foreignBusy || foreignSelected.size === 0}
            onclick={() => importForeign([...foreignSelected])}
            >Import selected ({foreignSelected.size})</Button
          >
          <Button
            variant="ghost"
            size="xs"
            disabled={foreignBusy}
            onclick={() => importForeign(foreignNames)}>Import all</Button
          >
        </div>
      </div>
      <div class="max-h-44 overflow-y-auto px-3 pb-2">
        <Table.Root class="text-xs">
          <Table.Body>
            {#each foreignNames as name}
              {@const sel = foreignSelected.has(name)}
              <Table.Row
                class="cursor-pointer {sel
                  ? 'bg-primary/10 hover:bg-primary/15'
                  : 'hover:bg-muted/50'}"
                onclick={() => toggleForeign(name)}
              >
                <Table.Cell class="w-8">
                  <input
                    type="checkbox"
                    checked={sel}
                    tabindex="-1"
                    onchange={() => toggleForeign(name)}
                    onclick={(e) => e.stopPropagation()}
                    class="size-3.5 accent-current"
                  />
                </Table.Cell>
                <Table.Cell class="max-w-md">
                  <div class="truncate font-mono" title={name}>{name}</div>
                </Table.Cell>
                <Table.Cell>
                  <div class="flex justify-end">
                    <Button
                      variant="outline"
                      size="xs"
                      disabled={foreignBusy}
                      onclick={(e) => {
                        e.stopPropagation();
                        importForeign([name]);
                      }}>Import</Button
                    >
                  </div>
                </Table.Cell>
              </Table.Row>
            {/each}
          </Table.Body>
        </Table.Root>
      </div>
    </div>
  {/if}

  {#if selectedIds.size > 0}
    <div
      class="h-9 shrink-0 flex items-center gap-2 px-3 bg-muted border-y text-xs"
    >
      <span class="font-medium">{selectedIds.size} selected</span>
      <div class="h-4 w-px bg-border"></div>
      <Button
        variant="outline"
        size="xs"
        onclick={async () => {
          for (const id of [...selectedIds])
            if (profile) profile.enabled[id] = true;
          if (profile) await api.saveProfile(profile);
          await refreshMods();
          await maybeAutoDeploy();
        }}>Enable</Button
      >
      <Button
        variant="outline"
        size="xs"
        onclick={async () => {
          for (const id of [...selectedIds])
            if (profile) profile.enabled[id] = false;
          if (profile) await api.saveProfile(profile);
          await refreshMods();
          await maybeAutoDeploy();
        }}>Disable</Button
      >
      <Button variant="destructive" size="xs" onclick={openRemoveConfirm}
        >Remove</Button
      >
      <Button variant="ghost" size="xs" class="ml-auto" onclick={handleClear}
        >Clear</Button
      >
    </div>
  {/if}

  <div class="h-10 shrink-0 flex items-center gap-2 px-3 border-t bg-muted/30">
    <Select.Root
      type="single"
      value={statusFilter}
      onValueChange={(v: string) => (statusFilter = v ?? "All")}
    >
      <Select.Trigger class="h-7 w-32.5 bg-background text-xs">
        <Select.Value placeholder="All" />
      </Select.Trigger>
      <Select.Content>
        <Select.Item value="All">All</Select.Item>
        <Select.Item value="Enabled">Enabled</Select.Item>
        <Select.Item value="Disabled">Disabled</Select.Item>
      </Select.Content>
    </Select.Root>
    <div class="flex-1 relative max-w-sm">
      <Input
        placeholder="Search mods…"
        bind:value={filter}
        class="h-7 pl-7 text-xs"
      />
      <span
        class="absolute left-2 top-1/2 -translate-y-1/2 text-muted-foreground text-xs"
        >⌕</span
      >
    </div>
    <span class="ml-auto hidden text-xs text-muted-foreground md:inline"
      >{filtered.length} shown</span
    >
    {#if isSorted}
      <button
        type="button"
        class="shrink-0 rounded-full border border-amber-500/40 px-2 py-0.5 text-[11px] font-medium text-amber-400 transition hover:bg-amber-500/10"
        title="Dragging is disabled while sorted — click to return to load order"
        onclick={() => {
          sortKey = "manual";
          sortDir = "asc";
          saveTableSort(MOD_SORT_KEY, { key: sortKey, dir: sortDir });
        }}
      >
        Reordering disabled
      </button>
    {/if}
  </div>

  <div class="h-21.5 shrink-0 border-t bg-card p-3">
    <div
      class="h-full rounded-lg border border-dashed flex flex-col items-center justify-center gap-1 text-xs transition {isDragging
        ? 'border-primary bg-primary/10 text-primary shadow-[0_0_28px_-8px_var(--color-ring)]'
        : 'bg-muted/10 text-muted-foreground hover:bg-muted/20 hover:text-foreground'}"
    >
      <span class="text-lg leading-none">⬇</span>
      <span
        >{isDragging ? "Drop to add" : "Drop .pak / .zip / folders here"}</span
      >
    </div>
  </div>

  <Dialog.Root bind:open={showStartupDialog}>
    <Dialog.Content class="sm:max-w-md">
      <Dialog.Header>
        <Dialog.Title>No Man's Sky not found</Dialog.Title>
        <Dialog.Description>
          We couldn't find your game automatically. Please select the folder
          that contains <span class="font-mono">GAMEDATA</span>.
        </Dialog.Description>
      </Dialog.Header>
      <div class="space-y-3 py-2">
        <div class="rounded-md border bg-muted/30 p-3 space-y-1.5">
          <div class="text-xs font-medium">Current path</div>
          <div
            class="font-mono text-xs break-all rounded bg-background border px-2 py-1.5 min-h-7 flex items-center"
          >
            {config?.game_path ?? "Auto-detect (not set)"}
          </div>
          {#if gameRoot}
            <div class="text-xs text-muted-foreground">
              Detected: <span class="font-mono break-all">{gameRoot}</span>
            </div>
          {:else}
            <div class="text-xs text-muted-foreground">
              Checked Steam libraries for <span class="font-mono">275850</span>
              — no <span class="font-mono">NMSARC.globals.pak</span> found.
            </div>
          {/if}
        </div>
        <p class="text-xs text-muted-foreground">
          Example: <span class="font-mono"
            >.../steamapps/common/No Man's Sky</span
          >
        </p>
      </div>
      <Dialog.Footer class="gap-2 sm:gap-2">
        <Button variant="ghost" onclick={() => (showStartupDialog = false)}
          >Skip</Button
        >
        <Button variant="outline" onclick={retryAutoDetect}
          >Retry auto-detect</Button
        >
        <Button onclick={pickGamePathStartup}>Choose folder…</Button>
      </Dialog.Footer>
    </Dialog.Content>
  </Dialog.Root>

  <Dialog.Root bind:open={showRemoveConfirm}>
    <Dialog.Content class="sm:max-w-md">
      <Dialog.Header>
        <Dialog.Title>Remove mods</Dialog.Title>
        <Dialog.Description>
          {removeConfirmCount === 1
            ? "Are you sure you want to remove this mod? This action cannot be undone."
            : `Are you sure you want to remove ${removeConfirmCount} mods? This action cannot be undone.`}
        </Dialog.Description>
      </Dialog.Header>
      <Dialog.Footer>
        <Button variant="ghost" onclick={() => (showRemoveConfirm = false)}
          >Cancel</Button
        >
        <Button variant="destructive" onclick={removeSelected}>Remove</Button>
      </Dialog.Footer>
    </Dialog.Content>
  </Dialog.Root>

  <Dialog.Root
    open={renameTarget !== null}
    onOpenChange={(o: boolean) => {
      if (!o) renameTarget = null;
    }}
  >
    <Dialog.Content class="sm:max-w-md">
      <Dialog.Header>
        <Dialog.Title>Rename mod</Dialog.Title>
        <Dialog.Description>
          Renames <span class="font-mono"
            >{renameTarget?.display_name ?? ""}</span
          > in your mod store.
        </Dialog.Description>
      </Dialog.Header>
      <Input
        bind:value={renameValue}
        bind:ref={renameInput}
        class="font-mono text-sm"
        placeholder="New mod name…"
        oninput={() => (renameUnchanged = false)}
        onkeydown={(e) => {
          if (e.key === "Enter") commitRenameDialog();
        }}
      />
      {#if renameUnchanged}
        <p class="text-xs text-amber-400">
          That's the current name — change it first.
        </p>
      {/if}
      {#if renameError}
        <p class="text-xs break-words text-destructive">{renameError}</p>
      {/if}
      <Dialog.Footer>
        <Button variant="ghost" onclick={() => (renameTarget = null)}
          >Cancel</Button
        >
        <Button
          onclick={commitRenameDialog}
          disabled={renameBusy || !renameValue.trim()}
        >
          {#if renameBusy}<LoaderCircle
              class="size-3.5 animate-spin"
            />Renaming…{:else}Rename{/if}
        </Button>
      </Dialog.Footer>
    </Dialog.Content>
  </Dialog.Root>
</div>
