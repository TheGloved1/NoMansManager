<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { LazyStore } from "@tauri-apps/plugin-store";
  import { api, type LogEntry } from "$lib/api";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Badge } from "$lib/components/ui/badge";
  import { Switch } from "$lib/components/ui/switch";
  import { Label } from "$lib/components/ui/label";
  import { ScrollText, Search, Trash2, FolderOpen } from "lucide-svelte";

  const settingsStore = new LazyStore("settings.json");

  let entries: LogEntry[] = $state([]);
  let sourceFilter: "all" | "mods" | "bases" | "app" = $state("all");
  let search = $state("");
  let autoScroll = $state(true);
  let maxLines = $state(2000);
  let confirmingClear = $state(false);
  let clearTimer: ReturnType<typeof setTimeout> | undefined;
  let scroller: HTMLDivElement | null = $state(null);

  function fmtTime(iso: string): string {
    const d = new Date(iso);
    return Number.isNaN(d.getTime()) ? iso : d.toLocaleTimeString([], { hour12: false });
  }

  let shown = $derived.by(() => {
    const q = search.trim().toLowerCase();
    return entries.filter(
      (e) =>
        (sourceFilter === "all" || e.source === sourceFilter) &&
        (!q || `${e.msg} ${e.source}`.toLowerCase().includes(q)),
    );
  });

  async function refresh() {
    try {
      entries = await api.readLogs();
    } catch {
      // log file may not exist yet — empty list is fine
    }
    try {
      const cap = await settingsStore.get<number>("logs_max_lines");
      if (typeof cap === "number" && cap > 0) maxLines = cap;
    } catch {}
  }

  $effect(() => {
    // keep pinned to the bottom on new entries when auto-scroll is on
    void shown.length;
    if (autoScroll && scroller) {
      queueMicrotask(() => {
        if (scroller) scroller.scrollTop = scroller.scrollHeight;
      });
    }
  });

  function armClear() {
    confirmingClear = true;
    cancelClearTimer();
    clearTimer = setTimeout(() => (confirmingClear = false), 3000);
  }
  function cancelClearTimer() {
    if (clearTimer !== undefined) {
      clearTimeout(clearTimer);
      clearTimer = undefined;
    }
  }
  async function doClear() {
    cancelClearTimer();
    confirmingClear = false;
    try {
      await api.clearLogs();
      entries = [];
    } catch {}
  }

  function levelMeta(level: string): { tag: string; cls: string } {
    if (level === "error") return { tag: "ERR", cls: "border-destructive/50 text-destructive" };
    if (level === "warn") return { tag: "WARN", cls: "border-amber-500/40 text-amber-400" };
    return { tag: "OK", cls: "border-emerald-500/40 text-emerald-400" };
  }

  onMount(() => {
    refresh();
    let unlisten: (() => void) | undefined;
    (async () => {
      try {
        unlisten = await listen("logs-changed", () => refresh());
      } catch {}
    })();
    return () => {
      cancelClearTimer();
      unlisten?.();
    };
  });
</script>

<div class="flex min-h-0 flex-1 flex-col overflow-hidden bg-background text-foreground">
  <div class="flex min-h-12 shrink-0 flex-wrap items-center gap-1.5 border-b border-border bg-card px-3 py-1.5">
    <div class="flex h-8 w-8 items-center justify-center rounded-md bg-primary text-primary-foreground">
      <ScrollText class="size-4" />
    </div>
    <div class="leading-tight">
      <div class="text-sm font-semibold tracking-tight">Logs</div>
      <div class="font-mono text-[11px] text-muted-foreground">
        {entries.length.toLocaleString()} / {maxLines.toLocaleString()} lines
      </div>
    </div>
    <div class="ml-auto flex items-center gap-2">
      <div class="flex items-center gap-1.5">
        <Label for="autoscroll" class="text-xs text-muted-foreground">Follow</Label>
        <Switch id="autoscroll" checked={autoScroll} onCheckedChange={(v) => (autoScroll = !!v)} />
      </div>
      <Button
        variant="outline"
        size="icon-sm"
        onclick={async () => {
          try {
            await api.openFolder(await api.getLogsDir());
          } catch {}
        }}
        title="Open log folder"
      >
        <FolderOpen class="size-3.5" />
      </Button>
      {#if confirmingClear}
        <Button variant="destructive" size="sm" onclick={doClear}>Sure?</Button>
      {:else}
        <Button variant="outline" size="sm" onclick={armClear} title="Clear the log file">
          <Trash2 class="size-3.5" />Clear
        </Button>
      {/if}
    </div>
  </div>

  <div class="flex shrink-0 items-center gap-2 border-b border-border px-3 py-2">
    <div class="grid grid-cols-4 gap-1 rounded-lg border border-border bg-background p-1">
      {#each [["all", "All"], ["mods", "Mods"], ["bases", "Bases"], ["app", "App"]] as [value, label]}
        <button
          class="rounded-md px-2 py-1 text-xs font-medium transition {sourceFilter === value
            ? 'bg-primary text-primary-foreground'
            : 'text-muted-foreground hover:bg-muted hover:text-foreground'}"
          onclick={() => (sourceFilter = value as typeof sourceFilter)}
        >
          {label}
        </button>
      {/each}
    </div>
    <div class="relative max-w-sm flex-1">
      <Search class="pointer-events-none absolute top-1/2 left-2 size-3.5 -translate-y-1/2 text-muted-foreground" />
      <Input bind:value={search} placeholder="Search log…" class="h-7 pl-7 text-xs" />
    </div>
    <span class="ml-auto hidden text-xs text-muted-foreground md:inline">{shown.length} shown</span>
  </div>

  <div bind:this={scroller} class="min-h-0 flex-1 overflow-auto px-3 py-2">
    {#if shown.length === 0}
      <div class="p-12 text-center">
        <div class="mx-auto max-w-sm space-y-2">
          <div class="text-sm font-medium">No log entries yet</div>
          <div class="text-xs text-muted-foreground">
            Every mod install, deploy, import, export, and backup lands here automatically.
          </div>
        </div>
      </div>
    {:else}
      <div class="flex flex-col gap-px font-mono text-xs">
        {#each shown as e}
          {@const meta = levelMeta(e.level)}
          <div class="flex items-baseline gap-2 rounded px-1.5 py-0.5 hover:bg-muted/50">
            <span class="shrink-0 text-muted-foreground">{fmtTime(e.ts)}</span>
            <Badge variant="outline" class="h-4 shrink-0 px-1 font-mono text-[10px] {meta.cls}">
              {meta.tag}
            </Badge>
            <Badge variant="outline" class="h-4 shrink-0 px-1 text-[10px]">{e.source}</Badge>
            <span class="min-w-0 flex-1 break-words whitespace-pre-wrap">{e.msg}</span>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>
