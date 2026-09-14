<script lang="ts">
  import { Card, CardContent, CardHeader, CardTitle } from "$lib/components/ui/card";
  import { Button } from "$lib/components/ui/button";
  import { buttonVariants } from "$lib/components/ui/button/button.svelte";
  import { Separator } from "$lib/components/ui/separator";
  import * as Dialog from "$lib/components/ui/dialog";
  import { cn } from "$lib/utils.js";

  let {
    isAdding,
    isDragging,
    onAddFiles,
    onAddFolder,
    onImport,
    onDeploy,
    onOpenMods,
    onOpenStore,
    deployStatus,
  }: {
    isAdding: boolean;
    isDragging: boolean;
    onAddFiles: () => void;
    onAddFolder: () => void;
    onImport: (move: boolean) => void;
    onDeploy: () => void;
    onOpenMods: () => void;
    onOpenStore: () => void;
    deployStatus: string;
  } = $props();

  let importOpen = $state(false);
</script>

<Card class="bg-nms-panel border-nms-border">
  <CardHeader class="p-3 pb-2">
    <CardTitle class="text-[11px] tracking-widest uppercase text-muted-foreground font-semibold">Library</CardTitle>
  </CardHeader>
  <CardContent class="p-3 pt-0 space-y-3">
    <div class="grid grid-cols-2 gap-2">
      <Button disabled={isAdding} onclick={onAddFiles}>{isAdding ? "Adding…" : "Add files"}</Button>
      <Button variant="outline" disabled={isAdding} onclick={onAddFolder}>Add folder</Button>
    </div>

    <div class="rounded-lg border-2 border-dashed p-3 text-center text-xs transition {isDragging ? 'border-primary bg-primary/10 text-primary' : 'border-nms-border text-muted-foreground'}">
      {isDragging ? "Drop to add" : "Drop .pak / .zip here"}
    </div>

    <Separator class="bg-nms-border" />

    <div class="flex items-center justify-between">
      <span class="text-[11px] font-semibold tracking-widest uppercase text-muted-foreground">Actions</span>
      {#if deployStatus}<span class="text-[11px] text-muted-foreground truncate max-w-[150px]">{deployStatus}</span>{/if}
    </div>

    <div class="grid grid-cols-2 gap-2">
      <Button class="col-span-2" onclick={onDeploy}>Deploy</Button>
      <Dialog.Root bind:open={importOpen}>
        <Dialog.Trigger class={cn(buttonVariants({ variant: "secondary", size: "sm" }), "col-span-2")}>
          Import existing mods
        </Dialog.Trigger>
        <Dialog.Content class="sm:max-w-md bg-popover">
          <Dialog.Header>
            <Dialog.Title>Import from MODS</Dialog.Title>
            <Dialog.Description>
              Copy mods already in <span class="font-mono">GAMEDATA/MODS</span> into the store. Choose how to handle the originals.
            </Dialog.Description>
          </Dialog.Header>
          <div class="grid gap-3 py-2">
            <Button
              variant="default"
              onclick={() => {
                importOpen = false;
                onImport(false);
              }}
            >
              Copy — keep originals
            </Button>
            <Button
              variant="outline"
              onclick={() => {
                importOpen = false;
                onImport(true);
              }}
            >
              Move — remove originals
            </Button>
          </div>
          <Dialog.Footer>
            <Button variant="ghost" onclick={() => (importOpen = false)}>Cancel</Button>
          </Dialog.Footer>
        </Dialog.Content>
      </Dialog.Root>
      <Button variant="ghost" size="sm" onclick={onOpenMods}>Open MODS</Button>
      <Button variant="ghost" size="sm" onclick={onOpenStore}>Open store</Button>
    </div>
  </CardContent>
</Card>
