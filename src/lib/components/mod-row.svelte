<script lang="ts">
  import { Badge } from "$lib/components/ui/badge";
  import { Checkbox } from "$lib/components/ui/checkbox";
  import type { Mod } from "$lib/types";

  let {
    mod,
    index,
    enabled,
    selected,
    onSelect,
    onToggle,
    sizeStr,
  }: {
    mod: Mod;
    index: number;
    enabled: boolean;
    selected: boolean;
    onSelect: (id: string, e: MouseEvent) => void;
    onToggle: (id: string) => void;
    sizeStr: (n: number) => string;
  } = $props();

  function handleRowClick(e: MouseEvent) {
    onSelect(mod.id, e);
  }
  function handleRowKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onSelect(mod.id, new MouseEvent("click"));
    }
  }
  function handleSelectToggle(checked: boolean) {
    const fake = new MouseEvent("click", { ctrlKey: true, bubbles: true } as any);
    onSelect(mod.id, fake);
  }
</script>

<div
  role="button"
  tabindex="0"
  data-selected={selected}
  class="group flex w-full items-center gap-3 px-4 py-2.5 text-left hover:bg-muted/60 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring {selected ? 'bg-muted' : ''} {enabled ? '' : 'opacity-55'}"
  onclick={handleRowClick}
  ondblclick={() => onToggle(mod.id)}
  onkeydown={handleRowKeydown}
>
  <span class="font-mono text-[11px] text-muted-foreground w-5 text-right shrink-0">{String(index + 1).padStart(2, "0")}</span>

  <div class="shrink-0" role="presentation" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
    <Checkbox
      checked={selected}
      onCheckedChange={(c) => handleSelectToggle(!!c)}
      aria-label="Select {mod.id}"
      class="h-4 w-4 rounded-[4px] border-muted-foreground/40 data-[state=checked]:bg-primary data-[state=checked]:text-primary-foreground"
    />
  </div>

  <div class="min-w-0 flex-1">
    <div class="truncate text-sm font-medium leading-none">{mod.display_name}</div>
    <div class="mt-1 flex items-center gap-2 text-[11px] text-muted-foreground">
      <Badge variant="outline" class="h-5 px-1.5 text-[10px]">{mod.type}</Badge>
      <span>{sizeStr(mod.size_bytes)}</span>
      {#if mod.has_lua}<Badge variant="secondary" class="h-5 text-[10px]">lua</Badge>{/if}
    </div>
  </div>

  <span class="hidden sm:inline text-[11px] {enabled ? 'text-transparent' : 'text-muted-foreground'}">{enabled ? "" : "off"}</span>
</div>
