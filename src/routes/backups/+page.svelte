<script lang="ts">
  import { onMount } from "svelte";
  import { api, type ManagedBackup, type SaveFileInfo } from "$lib/api";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Badge } from "$lib/components/ui/badge";
  import * as Table from "$lib/components/ui/table";
  import * as Dialog from "$lib/components/ui/dialog";
  import * as Select from "$lib/components/ui/select";
  import { ArchiveRestore, DatabaseBackup, Search, Trash2 } from "lucide-svelte";

  let items: ManagedBackup[] = $state([]);
  let kindFilter: "all" | "save" | "base" = $state("all");
  let search = $state("");
  let status = $state("");
  let backingUp = $state(false);

  let saveDir: string | null = $state(null);
  let saveFiles: SaveFileInfo[] = $state([]);

  let restoring: ManagedBackup | null = $state(null);
  let restoreTarget: string | null = $state(null);
  let restoringBusy = $state(false);
  let confirmDelete: string | null = $state(null);
  let confirmTimer: ReturnType<typeof setTimeout> | undefined;
  let confirmingClearAll = $state(false);
  let clearAllTimer: ReturnType<typeof setTimeout> | undefined;

  function totalSize(): string {
    const bytes = items.reduce((n, b) => n + b.size_bytes, 0);
    if (bytes > 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
    return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  }

  let shown = $derived.by(() => {
    const q = search.trim().toLowerCase();
    return items.filter(
      (b) =>
        (kindFilter === "all" || b.kind === kindFilter) &&
        (!q || b.name.toLowerCase().includes(q)),
    );
  });

  async function refresh() {
    try {
      items = await api.listAllBackups();
    } catch (e) {
      status = `List failed: ${e}`;
    }
  }

  async function detectSaves() {
    try {
      saveDir = await api.findSaveDir(null);
      if (saveDir) saveFiles = await api.listSaveFiles(saveDir);
    } catch {}
  }

  async function doBackupNow() {
    if (!saveDir) {
      await detectSaves();
      if (!saveDir) {
        status = "No save folder detected — open the Bases page and choose one";
        return;
      }
    }
    backingUp = true;
    try {
      const paths = await api.backupSaves(saveDir);
      status = `Backed up ${paths.length} save file(s)`;
      await refresh();
    } catch (e) {
      status = `Backup failed: ${e}`;
    } finally {
      backingUp = false;
    }
  }

  function armDelete(path: string) {
    confirmDelete = path;
    if (confirmTimer !== undefined) clearTimeout(confirmTimer);
    confirmTimer = setTimeout(() => (confirmDelete = null), 3000);
  }

  function armClearAll() {
    confirmingClearAll = true;
    if (clearAllTimer !== undefined) clearTimeout(clearAllTimer);
    clearAllTimer = setTimeout(() => (confirmingClearAll = false), 3000);
  }

  async function doClearAll() {
    if (clearAllTimer !== undefined) {
      clearTimeout(clearAllTimer);
      clearAllTimer = undefined;
    }
    confirmingClearAll = false;
    const targets = shown;
    if (!targets.length) return;
    let ok = 0;
    let firstErr: string | null = null;
    for (const b of targets) {
      try {
        await api.deleteBackup(b.path);
        ok++;
      } catch (e) {
        if (!firstErr) firstErr = `${b.name}: ${e}`;
      }
    }
    status = firstErr
      ? `Cleared ${ok}, ${targets.length - ok} failed — ${firstErr}`
      : `Cleared ${ok} backup(s)`;
    await refresh();
  }

  async function doDelete(path: string) {
    if (confirmTimer !== undefined) {
      clearTimeout(confirmTimer);
      confirmTimer = undefined;
    }
    confirmDelete = null;
    try {
      await api.deleteBackup(path);
      status = "Backup deleted";
      await refresh();
    } catch (e) {
      status = `Delete failed: ${e}`;
    }
  }

  function openRestore(b: ManagedBackup) {
    restoring = b;
    restoreTarget = saveFiles.length ? saveFiles[0].name : null;
  }

  async function doRestore() {
    if (!restoring || !saveDir || !restoreTarget) return;
    restoringBusy = true;
    try {
      await api.restoreSave(restoring.path, saveDir, restoreTarget);
      status = `Restored '${restoreTarget}' — load it on the Bases page to inspect`;
      restoring = null;
    } catch (e) {
      status = `Restore failed: ${e}`;
    } finally {
      restoringBusy = false;
    }
  }

  onMount(() => {
    refresh();
    detectSaves();
    return () => {
      if (confirmTimer !== undefined) clearTimeout(confirmTimer);
      if (clearAllTimer !== undefined) clearTimeout(clearAllTimer);
    };
  });
</script>

<div class="flex min-h-0 flex-1 flex-col overflow-hidden bg-background text-foreground">
  <div class="flex min-h-12 shrink-0 flex-wrap items-center gap-1.5 border-b border-border bg-card px-3 py-1.5">
    <div class="flex h-8 w-8 items-center justify-center rounded-md bg-primary text-primary-foreground">
      <DatabaseBackup class="size-4" />
    </div>
    <div class="leading-tight">
      <div class="text-sm font-semibold tracking-tight">Backups</div>
      <div class="font-mono text-[11px] text-muted-foreground">
        {items.length} files · {totalSize()}
      </div>
    </div>
    <div class="ml-auto flex items-center gap-1.5">
      {#if confirmingClearAll}
        <Button variant="destructive" size="sm" onclick={doClearAll}>
          Clear {shown.length}?
        </Button>
      {:else}
        <Button
          variant="outline"
          size="sm"
          onclick={armClearAll}
          disabled={shown.length === 0}
          title={kindFilter === "all"
            ? "Delete all backups"
            : `Delete shown ${kindFilter === "save" ? "save" : "base"} backups`}
        >
          <Trash2 class="size-3.5" />Clear{kindFilter === "all" ? "" : ` ${kindFilter === "save" ? "saves" : "bases"}`}
        </Button>
      {/if}
      <Button size="sm" onclick={doBackupNow} disabled={backingUp}>
        {#if backingUp}Backing up…{:else}<DatabaseBackup class="size-3.5" />Backup now{/if}
      </Button>
    </div>
  </div>

  <div class="flex shrink-0 items-center gap-2 border-b border-border px-3 py-2">
    <div class="grid grid-cols-3 gap-1 rounded-lg border border-border bg-background p-1">
      {#each [["all", "All"], ["save", "Saves"], ["base", "Bases"]] as [value, label]}
        <button
          class="rounded-md px-2 py-1 text-xs font-medium transition {kindFilter === value
            ? 'bg-primary text-primary-foreground'
            : 'text-muted-foreground hover:bg-muted hover:text-foreground'}"
          onclick={() => (kindFilter = value as typeof kindFilter)}
        >
          {label}
        </button>
      {/each}
    </div>
    <div class="relative max-w-sm flex-1">
      <Search class="pointer-events-none absolute top-1/2 left-2 size-3.5 -translate-y-1/2 text-muted-foreground" />
      <Input bind:value={search} placeholder="Search backups…" class="h-7 pl-7 text-xs" />
    </div>
    <span class="ml-auto hidden text-xs text-muted-foreground md:inline">{shown.length} shown</span>
  </div>

  <div class="min-h-0 flex-1 overflow-auto">
    {#if shown.length === 0}
      <div class="p-12 text-center">
        <div class="mx-auto max-w-sm space-y-2">
          <div class="text-sm font-medium">{items.length === 0 ? "No backups yet" : "No matches"}</div>
          <div class="text-xs text-muted-foreground">
            {items.length === 0
              ? "Backups are made automatically before every overwrite, import, and restore — or press Backup now."
              : "Try a different search or filter."}
          </div>
          {#if items.length === 0}
            <Button size="sm" class="mt-2" onclick={doBackupNow} disabled={backingUp}>
              <DatabaseBackup class="size-3.5" />Backup now
            </Button>
          {/if}
        </div>
      </div>
    {:else}
      <table class="w-full caption-bottom text-sm">
        <Table.Header class="sticky top-0 z-10 bg-muted">
          <Table.Row class="border-b border-border hover:bg-transparent">
            <Table.Head class="text-[11px] tracking-wide text-muted-foreground">Backup file</Table.Head>
            <Table.Head class="text-[11px] tracking-wide text-muted-foreground">Kind</Table.Head>
            <Table.Head class="text-right text-[11px] tracking-wide text-muted-foreground">Size</Table.Head>
            <Table.Head class="text-[11px] tracking-wide text-muted-foreground">Saved</Table.Head>
            <Table.Head class="text-right text-[11px] tracking-wide text-muted-foreground">Actions</Table.Head>
          </Table.Row>
        </Table.Header>
        <Table.Body>
          {#each shown as b (b.path)}
            <Table.Row class="hover:bg-muted/50">
              <Table.Cell class="max-w-md">
                <div class="truncate font-mono text-xs" title={b.path}>{b.name}</div>
              </Table.Cell>
              <Table.Cell>
                <Badge variant={b.kind === "save" ? "default" : "secondary"}>
                  {b.kind === "save" ? "Save" : "Base"}
                </Badge>
              </Table.Cell>
              <Table.Cell class="text-right font-mono text-xs text-muted-foreground">
                {b.size_display}
              </Table.Cell>
              <Table.Cell class="font-mono text-xs text-muted-foreground">{b.modified}</Table.Cell>
              <Table.Cell>
                <div class="flex justify-end gap-1.5">
                  {#if b.kind === "save"}
                    <Button variant="outline" size="xs" onclick={() => openRestore(b)}>
                      <ArchiveRestore class="size-3" />Restore
                    </Button>
                  {/if}
                  {#if confirmDelete === b.path}
                    <Button variant="destructive" size="xs" onclick={() => doDelete(b.path)}>
                      Sure?
                    </Button>
                  {:else}
                    <Button variant="ghost" size="xs" onclick={() => armDelete(b.path)} title="Delete backup">
                      <Trash2 class="size-3.5" />
                    </Button>
                  {/if}
                </div>
              </Table.Cell>
            </Table.Row>
          {/each}
        </Table.Body>
      </table>
    {/if}
  </div>
  <div class="shrink-0 truncate border-t border-border px-3 py-1 font-mono text-[11px] text-muted-foreground">
    {status || "Automatic backups are made before every overwrite, import, and restore."}
  </div>

  <Dialog.Root
    open={restoring !== null}
    onOpenChange={(o: boolean) => {
      if (!o) restoring = null;
    }}
  >
    <Dialog.Content class="max-w-md">
      <Dialog.Header>
        <Dialog.Title>Restore '{restoring?.name ?? ""}'?</Dialog.Title>
        <Dialog.Description>
          Writes over a live save. The current file is backed up again first — close NMS before restoring.
        </Dialog.Description>
      </Dialog.Header>
      {#if saveFiles.length}
        <div>
          <div class="mb-1 text-xs font-medium">Restore over</div>
          <Select.Root
            type="single"
            value={restoreTarget ?? ""}
            onValueChange={(v: string) => v && (restoreTarget = v)}
          >
            <Select.Trigger class="w-full font-mono text-xs">
              <Select.Value placeholder="Save file…" />
            </Select.Trigger>
            <Select.Content>
              {#each saveFiles as f}
                <Select.Item value={f.name}>{f.name} · {f.size_display}</Select.Item>
              {/each}
            </Select.Content>
          </Select.Root>
        </div>
      {:else}
        <p class="text-xs text-amber-400">
          No live saves detected — open the Bases page and choose a save folder first.
        </p>
      {/if}
      <Dialog.Footer>
        <Button variant="outline" onclick={() => (restoring = null)}>Cancel</Button>
        <Button variant="destructive" onclick={doRestore} disabled={!restoreTarget || restoringBusy}>
          {#if restoringBusy}Restoring…{:else}Restore{/if}
        </Button>
      </Dialog.Footer>
    </Dialog.Content>
  </Dialog.Root>
</div>
