<script lang="ts">
  import { Card, CardContent } from "$lib/components/ui/card";
  import { Button } from "$lib/components/ui/button";
  import { Badge } from "$lib/components/ui/badge";
  import { Separator } from "$lib/components/ui/separator";

  let {
    gameRoot,
    modsDir,
    globalDisabled,
    onPickPath,
    onToggleGlobal,
    onRefresh,
  }: {
    gameRoot: string | null;
    modsDir: string;
    globalDisabled: boolean;
    onPickPath: () => void;
    onToggleGlobal: () => void;
    onRefresh: () => void;
  } = $props();

  let label = $derived(gameRoot ? (gameRoot.split("/").pop() ?? gameRoot) : "Not found");
</script>

<Card class="bg-nms-panel border-nms-border">
  <CardContent class="p-3 space-y-3">
    <div class="flex items-center justify-between">
      <div class="flex items-center gap-2 min-w-0">
        <span class="h-2 w-2 rounded-full {gameRoot ? 'bg-nms-success shadow-[0_0_6px_rgba(0,230,118,0.4)]' : 'bg-nms-danger'}"></span>
        <span class="text-xs font-medium truncate">{gameRoot ? "Game found" : "No game"}</span>
        {#if gameRoot}
          <span class="text-xs text-muted-foreground truncate max-w-[150px]">— {label}</span>
        {/if}
      </div>
      <Button variant="ghost" size="xs" onclick={onPickPath}>Settings</Button>
    </div>
    {#if modsDir}
      <div class="text-[11px] font-mono text-muted-foreground truncate" title={modsDir}>{modsDir}</div>
    {/if}
    <Separator class="bg-nms-border" />
    <div class="flex items-center justify-between">
      <div class="flex items-center gap-2">
        <Badge variant={globalDisabled ? "destructive" : "secondary"} class="text-[10px] leading-none">
          {globalDisabled ? "All disabled" : "Enabled"}
        </Badge>
        {#if globalDisabled}
          <span class="text-[11px] text-muted-foreground">DISABLEMODS.txt</span>
        {/if}
      </div>
      <div class="flex gap-1">
        <Button variant="ghost" size="xs" onclick={onToggleGlobal}>
          {globalDisabled ? "Enable" : "Disable all"}
        </Button>
        <Button variant="ghost" size="xs" onclick={onRefresh}>Refresh</Button>
      </div>
    </div>
  </CardContent>
</Card>
