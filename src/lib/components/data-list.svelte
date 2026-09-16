<script lang="ts" module>
  export interface DataListColumn {
    id: string;
    label: string;
    /** Show sort control and arrow affordance. */
    sortable?: boolean;
    align?: "left" | "right";
  }
</script>

<script lang="ts" generics="T">
  import type { Snippet } from "svelte";
  import SortHeader from "./sort-header.svelte";
  import type { SortDir } from "$lib/table-sort";

  interface Props {
    /** Column definitions (header row). */
    columns: DataListColumn[];
    /** Explicit grid template shared by header and rows, e.g. "auto minmax(0,1fr) auto auto". */
    gridTemplate: string;
    items: T[];
    keyOf: (item: T) => string | number;
    isSelected: (item: T) => boolean;
    sortKey?: string | null;
    sortDir?: SortDir;
    onSort?: (id: string) => void;
    onSelect?: (item: T, e: MouseEvent | KeyboardEvent) => void;
    /** Double-click / Enter on a focused row. */
    onActivate?: (item: T) => void;
    onBackgroundClear?: () => void;
    /** Return false to disable dragging for an item (e.g. while sorted). */
    isDraggable?: (item: T) => boolean;
    onReorder?: (
      fromKey: string | number,
      toKey: string | number,
      pos: "before" | "after",
    ) => void;
    /** Row cells for one item. Parent owns actions (stopPropagation included). */
    row: Snippet<[item: T, selected: boolean]>;
    /** Shown when items is empty (loading / no data / no matches). */
    empty?: Snippet;
  }

  let {
    columns,
    gridTemplate,
    items,
    keyOf,
    isSelected,
    sortKey = null,
    sortDir = "asc",
    onSort,
    onSelect,
    onActivate,
    onBackgroundClear,
    isDraggable,
    onReorder,
    row,
    empty,
  }: Props = $props();

  let dragFromKey: string | number | null = $state(null);
  let dragOverKey: string | number | null = $state(null);
  let dragOverPos: "before" | "after" | null = $state(null);

  function clearDrag() {
    dragFromKey = null;
    dragOverKey = null;
    dragOverPos = null;
  }
</script>

<div
  class="min-h-0 flex-1 overflow-auto"
  role="button"
  tabindex="0"
  onclick={(e) => {
    if (e.target === e.currentTarget) onBackgroundClear?.();
  }}
  onkeydown={(e) => {
    if ((e.key === "Enter" || e.key === " ") && e.target === e.currentTarget) {
      e.preventDefault();
      onBackgroundClear?.();
    }
  }}
>
  <div class="min-w-0">
    <div
      class="sticky top-0 z-10 grid gap-2 border-b border-border bg-muted px-3 py-2 text-[11px] font-medium tracking-wide text-muted-foreground {gridTemplate}"
    >
      {#each columns as col}
        {#if col.sortable && onSort}
          <SortHeader
            label={col.label}
            active={sortKey === col.id}
            dir={sortDir}
            align={col.align ?? "left"}
            onclick={() => onSort(col.id)}
          />
        {:else}
          <div class={col.align === "right" ? "text-right uppercase" : "uppercase"}>{col.label}</div>
        {/if}
      {/each}
    </div>
    {#if items.length === 0}
      {@render empty?.()}
    {:else}
      <div class="divide-y">
        {#each items as item (keyOf(item))}
          {@const selected = isSelected(item)}
          {@const draggable = isDraggable?.(item) ?? false}
          {@const isOver = dragOverKey !== null && dragOverKey === keyOf(item)}
          <div
            role="button"
            tabindex="0"
            draggable={draggable}
            ondragstart={(e) => {
              if (!draggable) return;
              dragFromKey = keyOf(item);
              if (e.dataTransfer) {
                e.dataTransfer.effectAllowed = "move";
                e.dataTransfer.setData("text/plain", String(keyOf(item)));
              }
            }}
            ondragover={(e) => {
              if (dragFromKey === null || dragFromKey === keyOf(item)) return;
              e.preventDefault();
              const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
              dragOverKey = keyOf(item);
              dragOverPos = e.clientY < r.top + r.height / 2 ? "before" : "after";
            }}
            ondrop={(e) => {
              e.preventDefault();
              if (dragFromKey !== null && dragFromKey !== keyOf(item))
                onReorder?.(dragFromKey, keyOf(item), dragOverPos ?? "before");
              clearDrag();
            }}
            ondragend={clearDrag}
            onclick={(e) => onSelect?.(item, e)}
            onkeydown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                onSelect?.(item, e);
              }
            }}
            ondblclick={() => onActivate?.(item)}
            class="relative grid cursor-pointer items-center gap-2 px-3 py-2 text-sm {gridTemplate} {selected
              ? "bg-primary/10 hover:bg-primary/20"
              : "hover:bg-muted/50"} {keyOf(item) === dragFromKey ? "opacity-40" : ""}"
          >
            {#if isOver && dragOverPos === "before"}
              <div class="absolute inset-x-0 top-0 h-0.5 bg-primary"></div>
            {/if}
            {#if isOver && dragOverPos === "after"}
              <div class="absolute inset-x-0 bottom-0 h-0.5 bg-primary"></div>
            {/if}
            {@render row(item, selected)}
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>
