<script lang="ts">
  import { Card, CardContent, CardHeader, CardTitle } from "$lib/components/ui/card";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Badge } from "$lib/components/ui/badge";
  import { Separator } from "$lib/components/ui/separator";
  import * as Select from "$lib/components/ui/select";

  let {
    profiles,
    active,
    modsCount,
    enabledCount,
    onSwitch,
    onCreate,
    onDelete,
  }: {
    profiles: string[];
    active: string | null;
    modsCount: number;
    enabledCount: number;
    onSwitch: (name: string) => void;
    onCreate: (name: string) => void;
    onDelete: (name: string) => void;
  } = $props();

  let showInput = $state(false);
  let draft = $state("");

  function submit() {
    if (!draft.trim()) return;
    onCreate(draft.trim());
    draft = "";
    showInput = false;
  }
</script>

<Card class="bg-nms-panel border-nms-border">
  <CardHeader class="p-3 pb-2 space-y-0">
    <CardTitle class="text-[11px] tracking-widest uppercase text-muted-foreground font-semibold">Profile</CardTitle>
  </CardHeader>
  <CardContent class="p-3 pt-0 space-y-3">
    <div class="flex gap-2">
      <Select.Root type="single" value={active ?? ""} onValueChange={(v) => v && onSwitch(v)}>
        <Select.Trigger class="flex-1 h-8 bg-background">
          <Select.Value placeholder="Select profile" />
        </Select.Trigger>
        <Select.Content>
          {#each profiles as p}
            <Select.Item value={p}>{p}</Select.Item>
          {/each}
        </Select.Content>
      </Select.Root>
      <Button variant="outline" size="icon-sm" onclick={() => (showInput = !showInput)} aria-label="New profile">＋</Button>
    </div>

    {#if showInput}
      <div class="flex gap-2">
        <Input class="h-8" placeholder="name" bind:value={draft} onkeydown={(e) => e.key === 'Enter' && submit()} />
        <Button size="sm" onclick={submit}>Create</Button>
      </div>
    {/if}

    <div class="flex flex-wrap gap-1.5">
      {#each profiles as p}
        <Badge variant={p === active ? "default" : "secondary"} class="gap-1 pr-1 text-xs">
          {p}
          {#if profiles.length > 1}
            <button class="ml-1 rounded-full hover:bg-black/20 px-1 -mr-1" onclick={() => onDelete(p)} aria-label="Delete {p}">×</button>
          {/if}
        </Badge>
      {/each}
    </div>

    <Separator class="bg-nms-border" />
    <div class="text-xs text-muted-foreground">{modsCount} mods • {enabledCount} enabled</div>
  </CardContent>
</Card>
