<script lang="ts">
  import ModRow from "./mod-row.svelte";
  import { Button } from "$lib/components/ui/button";
  import { Card } from "$lib/components/ui/card";
  import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "$lib/components/ui/empty";
  import { Separator } from "$lib/components/ui/separator";
  import type { Mod, Profile } from "$lib/types";

  let {
    mods,
    profile,
    selectedIds,
    deployedMap,
    onSelect,
    onToggle,
    onMoveUp,
    onMoveDown,
    onRemove,
    onAdd,
    onReorder,
    onAutoSort,
    onClear,
    sizeStr,
  }: {
    mods: Mod[];
    profile: Profile | null;
    selectedIds: Set<string>;
    deployedMap: Record<string, string>;
    onSelect: (id: string, e: MouseEvent) => void;
    onToggle: (id: string) => void;
    onMoveUp: () => void;
    onMoveDown: () => void;
    onRemove: (id: string) => void;
    onAdd: () => void;
    onReorder: (fromId: string, toId: string, pos: "before" | "after") => void;
    onAutoSort: () => void;
    onClear: () => void;
    sizeStr: (n: number) => string;
  } = $props();

  let draggedId: string | null = $state(null);
  let dragOverId: string | null = $state(null);
  let dragOverPos: "before" | "after" | null = $state(null);
  // keep deployedMap for future use (e.g., deployed indicator) — reference inside effect to avoid unused warning
  $effect(() => { void deployedMap; });

  function handleDragStart(e: DragEvent, id: string) {
    draggedId = id;
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = "move";
      e.dataTransfer.setData("text/plain", id);
    }
  }
  function handleDragOver(e: DragEvent, id: string) {
    e.preventDefault();
    if (!draggedId || draggedId === id) return;
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const mid = rect.top + rect.height / 2;
    dragOverId = id;
    dragOverPos = e.clientY < mid ? "before" : "after";
  }
  function handleDrop(e: DragEvent, targetId: string) {
    e.preventDefault();
    if (draggedId && draggedId !== targetId) {
      const pos = dragOverPos ?? "before";
      onReorder(draggedId, targetId, pos);
    }
    draggedId = null;
    dragOverId = null;
    dragOverPos = null;
  }
  function handleDragEnd() {
    draggedId = null;
    dragOverId = null;
    dragOverPos = null;
  }
</script>

<Card class="flex min-h-0 flex-1 flex-col overflow-hidden bg-card border-nms-border gap-0 py-0">
  <div class="flex h-10 shrink-0 items-center justify-between border-b bg-card px-4">
    <div class="flex items-center gap-2">
      <span class="flex h-6 w-6 items-center justify-center rounded-md border bg-muted text-[11px]">{mods.length}</span>
      <span class="text-sm font-medium">Mods</span>
      {#if selectedIds.size === 1}<span class="hidden md:inline text-xs text-muted-foreground">• {[...selectedIds][0]}</span>
      {:else if selectedIds.size > 1}<span class="hidden md:inline text-xs text-muted-foreground">• {selectedIds.size} selected</span>
      {/if}
    </div>
    <div class="flex items-center gap-1.5">
      <Button variant="ghost" size="sm" onclick={onAutoSort} title="Auto-sort by contents (pak vs folder, lua, name)">Auto</Button>
      <Separator orientation="vertical" class="h-4" />
      <Button variant="outline" size="sm" disabled={selectedIds.size === 0} onclick={async () => { for (const id of [...selectedIds]) await onToggle(id); }}>Toggle</Button>
      <Separator orientation="vertical" class="h-4" />
      <Button variant="ghost" size="icon-sm" onclick={onMoveUp} aria-label="Up" disabled={selectedIds.size !== 1}>↑</Button>
      <Button variant="ghost" size="icon-sm" onclick={onMoveDown} aria-label="Down" disabled={selectedIds.size !== 1}>↓</Button>
      <Button variant="ghost" size="sm" class="text-destructive hover:text-destructive hover:bg-destructive/10" disabled={selectedIds.size === 0} onclick={async () => { const ids = [...selectedIds]; for (const id of ids) await onRemove(id); }}>Remove</Button>
    </div>
  </div>

  <div
    class="flex-1 overflow-auto focus:outline-none"
    role="button"
    tabindex="0"
    aria-label="Mod list empty space — click to clear selection"
    onclick={(e) => { if (e.target === e.currentTarget) onClear(); }}
    onkeydown={(e) => { if (e.key === 'Escape' || e.key === 'Enter' || e.key === ' ') onClear(); }}
  >
    {#if mods.length === 0}
      <Empty class="h-full">
        <EmptyMedia variant="icon">◇</EmptyMedia>
        <EmptyHeader>
          <EmptyTitle>No mods yet</EmptyTitle>
          <EmptyDescription>Add .pak or .zip files, or import from your game’s MODS folder.</EmptyDescription>
        </EmptyHeader>
        <EmptyContent>
          <Button onclick={onAdd}>Add mods</Button>
        </EmptyContent>
      </Empty>
    {:else}
      <div class="divide-y" role="list">
        {#each mods as mod, idx}
          {@const isDragging = draggedId === mod.id || (draggedId && selectedIds.has(draggedId) && selectedIds.has(mod.id))}
          {@const isOver = dragOverId === mod.id}
          <div
            role="listitem"
            data-selected={selectedIds.has(mod.id)}
            draggable="true"
            ondragstart={(e) => handleDragStart(e, mod.id)}
            ondragover={(e) => handleDragOver(e, mod.id)}
            ondrop={(e) => handleDrop(e, mod.id)}
            ondragend={handleDragEnd}
            ondragleave={() => { if (dragOverId === mod.id) { dragOverId = null; dragOverPos = null; } }}
            class="relative flex items-center gap-1 transition {isDragging ? 'opacity-40 scale-[0.98]' : ''}"
          >
            {#if isOver && dragOverPos === 'before'}
              <div class="pointer-events-none absolute inset-x-0 top-0 z-10">
                <div class="h-0.5 w-full bg-primary shadow-[0_0_8px_hsl(var(--primary))]"></div>
                <div class="absolute -left-1 top-0 h-2 w-2 -translate-y-1/2 rounded-full bg-primary"></div>
              </div>
            {/if}
            {#if isOver && dragOverPos === 'after'}
              <div class="pointer-events-none absolute inset-x-0 bottom-0 z-10">
                <div class="h-0.5 w-full bg-primary shadow-[0_0_8px_hsl(var(--primary))]"></div>
                <div class="absolute -left-1 bottom-0 h-2 w-2 translate-y-1/2 rounded-full bg-primary"></div>
              </div>
            {/if}
            <span
              class="ml-2 flex h-6 w-4 shrink-0 cursor-grab items-center justify-center rounded text-muted-foreground hover:text-foreground active:cursor-grabbing"
              title="Drag to reorder"
              aria-hidden="true"
            >⋮⋮</span>
            <div class="min-w-0 flex-1">
              <ModRow
                {mod}
                index={idx}
                enabled={profile?.enabled[mod.id] ?? true}
                selected={selectedIds.has(mod.id)}
                {onSelect}
                onToggle={onToggle}
                {sizeStr}
              />
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</Card>
