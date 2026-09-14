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
  let globalDisabled = $state(false);
  let status = $state("Ready");
  let isAdding = $state(false);
  let isDragging = $state(false);
  let filter = $state("");
  let statusFilter: string = $state("All");
  let lastAddKey = "";
  let lastAddAt = 0;
  let draggedId: string | null = $state(null);
  let dragOverId: string | null = $state(null);
  let dragOverPos: "before" | "after" | null = $state(null);

  function sizeStr(b: number) {
    if (b > 1024 * 1024) return `${(b / 1024 / 1024).toFixed(1)} MB`;
    if (b > 1024) return `${(b / 1024).toFixed(0)} KB`;
    return `${b} B`;
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
        deployedMap = await api.scanDeployed(modsDir);
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
    if (modsDir)
      try {
        deployedMap = await api.scanDeployed(modsDir);
      } catch {}
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
    deployedMap = await api.scanDeployed(modsDir);
    await refreshMods();
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
    if (!confirm(`Remove ${selectedIds.size} mod(s)?`)) return;
    for (const id of [...selectedIds]) {
      profile.mod_order = profile.mod_order.filter((x) => x !== id);
      delete profile.enabled[id];
      await api.removeStoreMod(id);
    }
    await api.saveProfile(profile);
    selectedIds = new Set();
    await refreshMods();
  }


  let filtered = $derived(
    mods.filter((m) => {
      const q = filter.toLowerCase();
      const matchesName = !q || m.display_name.toLowerCase().includes(q);
      const enabled = profile?.enabled[m.id] ?? true;
      const matchesStatus =
        statusFilter === "All" ||
        (statusFilter === "Enabled" && enabled) ||
        (statusFilter === "Disabled" && !enabled);
      return matchesName && matchesStatus;
    }),
  );

  onMount(() => {
    let cleanup: (() => void) | undefined;
    (async () => {
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
      if (e.key === "Escape") {
        if (showStartupDialog) showStartupDialog = false;
        else handleClear();
      }
      if ((e.key === "Delete" || e.key === "Backspace") && selectedIds.size)
        removeSelected();
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
  <div class="h-12 shrink-0 flex items-center gap-1 px-3 border-b bg-card">
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
    <Button variant="outline" size="sm" onclick={doPurge}>Purge</Button>
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

  {#if selectedIds.size > 0}
    <div
      class="h-9 shrink-0 flex items-center gap-2 px-3 bg-muted border-b text-xs"
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
        }}>Disable</Button
      >
      <Button variant="destructive" size="xs" onclick={removeSelected}
        >Remove</Button
      >
      <Button variant="ghost" size="xs" class="ml-auto" onclick={handleClear}
        >Clear</Button
      >
    </div>
  {/if}

  <div class="h-10 shrink-0 flex items-center gap-2 px-3 border-b bg-muted/30">
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
    <span class="hidden md:inline text-xs text-muted-foreground ml-auto"
      >{filtered.length} shown</span
    >
  </div>

  <div
    class="flex-1 overflow-auto"
    role="button"
    tabindex="0"
    onclick={(e) => {
      if (e.target === e.currentTarget) handleClear();
    }}
    onkeydown={(e) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        handleClear();
      }
    }}
  >
    <div class="min-w-160">
      <div
        class="sticky top-0 z-10 grid grid-cols-[140px_1fr_90px_120px] gap-2 px-3 py-2 bg-muted border-b text-[11px] font-medium tracking-wide text-muted-foreground"
      >
        <div>Status</div>
        <div>Mod Name</div>
        <div>Size</div>
        <div class="text-right">Actions</div>
      </div>
      {#if filtered.length === 0}
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
      {:else}
        <div class="divide-y">
          {#each filtered as mod}
            {@const enabled = profile?.enabled[mod.id] ?? true}
            {@const isSel = selectedIds.has(mod.id)}
            {@const isOver = dragOverId === mod.id}
            {@const isDrag =
              draggedId === mod.id ||
              (draggedId &&
                selectedIds.has(draggedId) &&
                selectedIds.has(mod.id))}
            <div
              role="button"
              tabindex="0"
              draggable="true"
              ondragstart={(e) => {
                draggedId = mod.id;
                if (e.dataTransfer) {
                  e.dataTransfer.effectAllowed = "move";
                  e.dataTransfer.setData("text/plain", mod.id);
                }
              }}
              ondragover={(e) => {
                e.preventDefault();
                if (!draggedId || draggedId === mod.id) return;
                const r = (
                  e.currentTarget as HTMLElement
                ).getBoundingClientRect();
                const m = r.top + r.height / 2;
                dragOverId = mod.id;
                dragOverPos = e.clientY < m ? "before" : "after";
              }}
              ondrop={(e) => {
                e.preventDefault();
                if (draggedId && draggedId !== mod.id)
                  handleReorder(draggedId, mod.id, dragOverPos ?? "before");
                draggedId = null;
                dragOverId = null;
              }}
              ondragend={() => {
                draggedId = null;
                dragOverId = null;
              }}
              onclick={(e) => handleSelect(mod.id, e as MouseEvent)}
              onkeydown={(e) => {
                if (e.key === 'Enter' || e.key === ' ') {
                  e.preventDefault();
                  handleSelect(mod.id, e as unknown as MouseEvent);
                }
              }}
              ondblclick={() => {
                const cur = profile?.enabled[mod.id] ?? true;
                if (profile) {
                  profile.enabled[mod.id] = !cur;
                  api.saveProfile(profile).then(() => refreshMods());
                }
              }}
              class="relative grid grid-cols-[140px_1fr_90px_120px] gap-2 items-center px-3 py-2 text-sm cursor-pointer hover:bg-muted/50 {isSel
                ? 'bg-primary/10'
                : ''} {enabled ? '' : 'opacity-60'} {isDrag
                ? 'opacity-40'
                : ''}"
            >
              {#if isOver && dragOverPos === "before"}<div
                  class="absolute inset-x-0 top-0 h-0.5 bg-primary"
                ></div>{/if}
              {#if isOver && dragOverPos === "after"}<div
                  class="absolute inset-x-0 bottom-0 h-0.5 bg-primary"
                ></div>{/if}
              <div class="flex items-center gap-2">
                <span
                  class="inline-flex items-center rounded-full border px-2 py-0.5 text-[11px] font-medium {enabled
                    ? 'bg-primary text-primary-foreground border-primary'
                    : 'bg-muted text-muted-foreground'}"
                >
                  {enabled ? "Enabled" : "Disabled"}
                </span>
              </div>
              <div class="truncate font-medium" title={mod.display_name}>
                {mod.display_name}
              </div>
              <div class="text-xs text-muted-foreground">
                {sizeStr(mod.size_bytes)}
              </div>
              <div class="flex justify-end">
                <Button
                  variant="outline"
                  size="xs"
                  onclick={(e) => {
                    e.stopPropagation();
                    handleSelect(mod.id, new MouseEvent("click"));
                    removeSelected();
                  }}>Remove</Button
                >
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </div>

  <div class="h-21.5 shrink-0 border-t bg-card p-3">
    <div
      class="h-full rounded-lg border border-dashed flex flex-col items-center justify-center gap-1 text-xs {isDragging
        ? 'border-primary bg-primary/10 text-primary'
        : 'border-muted-foreground/20 text-muted-foreground bg-muted/20'}"
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
</div>
