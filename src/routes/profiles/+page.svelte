<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import { loadConfigNative, saveConfigNative } from '$lib/config';
  import type { Profile } from '$lib/types';
  import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '$lib/components/ui/card';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Badge } from '$lib/components/ui/badge';
  import { Separator } from '$lib/components/ui/separator';
  import * as Select from '$lib/components/ui/select';

  let profiles: string[] = $state([]);
  let active: string | null = $state(null);
  let modsCount = $state(0);
  let newName = $state("");
  let renameTarget: string | null = $state(null);
  let renameValue = $state("");

  async function refresh() {
    profiles = await api.listProfiles();
    const cfg = await loadConfigNative();
    active = cfg.active_profile;
    // get mods count for header
    const mods = await api.scanStore();
    modsCount = mods.length;
  }

  onMount(refresh);

  async function create() {
    const n = newName.trim().replace(/\s+/g, "_");
    if (!n) return;
    await api.createProfile(n, active);
    newName = "";
    await refresh();
  }

  async function switchTo(name: string) {
    const cfg = await loadConfigNative();
    cfg.active_profile = name;
    await saveConfigNative(cfg);
    active = name;
    await refresh();
  }

  async function remove(name: string) {
    if (profiles.length <= 1) return;
    if (!confirm(`Delete profile '${name}'?`)) return;
    await api.deleteProfile(name);
    await refresh();
    if (active === name) {
      const cfg = await loadConfigNative();
      active = cfg.active_profile;
    }
  }

  async function startRename(name: string) {
    renameTarget = name;
    renameValue = name;
  }

  async function confirmRename() {
    if (!renameTarget || !renameValue.trim() || renameTarget === renameValue.trim()) {
      renameTarget = null;
      return;
    }
    const next = renameValue.trim().replace(/\s+/g, "_");
    await api.renameProfile(renameTarget, next);
    if (active === renameTarget) {
      const cfg = await loadConfigNative();
      cfg.active_profile = next;
      await saveConfigNative(cfg);
    }
    renameTarget = null;
    await refresh();
  }

  async function duplicate(name: string) {
    const copy = `${name}_copy`;
    await api.createProfile(copy, name);
    await refresh();
  }
</script>

<div class="flex flex-1 flex-col bg-background overflow-hidden">
  <div class="h-12 shrink-0 flex items-center justify-between border-b bg-card px-5">
    <div>
      <div class="text-sm font-semibold">Profiles</div>
      <div class="text-xs text-muted-foreground">{profiles.length} profiles • {modsCount} mods in store</div>
    </div>
  </div>

  <div class="flex-1 overflow-auto p-6">
    <div class="mx-auto max-w-[720px] space-y-6">
      <Card>
        <CardHeader>
          <CardTitle class="text-sm">Active profile</CardTitle>
          <CardDescription>Choose which mod set is deployed to the game. Deploy still required to apply.</CardDescription>
        </CardHeader>
        <CardContent class="space-y-3">
          <Select.Root type="single" value={active ?? ""} onValueChange={(v) => v && switchTo(v)}>
            <Select.Trigger class="w-full bg-background">
              <Select.Value placeholder="Select profile" />
            </Select.Trigger>
            <Select.Content>
              {#each profiles as p}
                <Select.Item value={p}>{p} {#if p === active}• active{/if}</Select.Item>
              {/each}
            </Select.Content>
          </Select.Root>
          {#if active}
            <div class="text-xs text-muted-foreground">Active: <span class="font-mono font-medium text-foreground">{active}</span></div>
          {/if}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle class="text-sm">Create profile</CardTitle>
          <CardDescription>New profiles copy mods and order from the active one. You can reorder and enable/disable per profile.</CardDescription>
        </CardHeader>
        <CardContent class="flex gap-2">
          <Input placeholder="my-new-profile" bind:value={newName} onkeydown={(e) => e.key === 'Enter' && create()} class="flex-1" />
          <Button onclick={create} disabled={!newName.trim()}>Create</Button>
        </CardContent>
      </Card>

      <div class="space-y-3">
        <div class="text-xs font-semibold tracking-widest text-muted-foreground uppercase">All profiles</div>
        {#each profiles as p}
          <Card class="overflow-hidden {p === active ? 'border-primary' : ''}">
            <CardContent class="p-3 flex items-center gap-3">
              <div class="flex-1 min-w-0">
                {#if renameTarget === p}
                  <div class="flex gap-2">
                    <Input bind:value={renameValue} class="h-7 text-sm flex-1" onkeydown={(e) => { if (e.key === 'Enter') confirmRename(); if (e.key === 'Escape') renameTarget = null; }} />
                    <Button size="sm" class="h-7" onclick={confirmRename}>Save</Button>
                    <Button variant="ghost" size="sm" class="h-7" onclick={() => renameTarget = null}>Cancel</Button>
                  </div>
                {:else}
                  <div class="flex items-center gap-2">
                    <span class="font-medium text-sm truncate">{p}</span>
                    {#if p === active}<Badge variant="default" class="h-5 text-[10px]">Active</Badge>{/if}
                  </div>
                  <div class="text-xs text-muted-foreground">{p === active ? 'Currently deployed profile' : 'Inactive'}</div>
                {/if}
              </div>

              <div class="flex items-center gap-1 shrink-0">
                {#if renameTarget !== p}
                  <Button variant="ghost" size="xs" onclick={() => switchTo(p)} disabled={p === active}>Activate</Button>
                  <Button variant="ghost" size="xs" onclick={() => duplicate(p)}>Duplicate</Button>
                  <Button variant="ghost" size="xs" onclick={() => startRename(p)}>Rename</Button>
                  <Button variant="ghost" size="xs" class="text-destructive hover:text-destructive" disabled={profiles.length <= 1} onclick={() => remove(p)}>Delete</Button>
                {/if}
              </div>
            </CardContent>
          </Card>
        {/each}
      </div>

      <Card class="border-dashed">
        <CardContent class="p-4 flex items-center justify-between">
          <div class="text-xs text-muted-foreground">Profiles are stored in <span class="font-mono">~/.local/share/nms-mod-manager/profiles</span></div>
          <Button variant="outline" size="xs" onclick={refresh}>Refresh</Button>
        </CardContent>
      </Card>
    </div>
  </div>
</div>
