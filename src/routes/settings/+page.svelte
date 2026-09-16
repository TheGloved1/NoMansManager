<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { open } from '@tauri-apps/plugin-dialog';
  import { api } from '$lib/api';
  import { loadConfigNative, saveConfigNative } from '$lib/config';
  import type { AppConfig } from '$lib/types';
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '$lib/components/ui/card';
import { Button } from '$lib/components/ui/button';
import { Label } from '$lib/components/ui/label';
import * as Select from '$lib/components/ui/select';
import * as Dialog from '$lib/components/ui/dialog';
import { Separator } from '$lib/components/ui/separator';

  const themes = [
    { id: 'default', label: 'Default (NMS Dark)', desc: 'Space blue — default' },
    { id: 'rose-pine', label: 'Rose Pine', desc: 'Muted pine, love pink' },
    { id: 'rose-pine-moon', label: 'Rose Pine Moon', desc: 'Darker violet' },
    { id: 'rose-pine-dawn', label: 'Rose Pine Dawn', desc: 'Warm light' },
    { id: 'catppuccin-mocha', label: 'Catppuccin Mocha', desc: 'Rich dark' },
    { id: 'catppuccin-macchiato', label: 'Catppuccin Macchiato', desc: 'Soft dark' },
    { id: 'catppuccin-frappe', label: 'Catppuccin Frappé', desc: 'Muted mid' },
    { id: 'catppuccin-latte', label: 'Catppuccin Latte', desc: 'Bright light' },
  ];

  const fonts = [
    { id: 'inter', label: 'Inter', desc: 'Clean sans — default' },
    { id: 'geist', label: 'Geist Sans', desc: 'Geometric' },
    { id: 'space', label: 'Space Grotesk', desc: 'Futuristic' },
    { id: 'manrope', label: 'Manrope', desc: 'Friendly' },
    { id: 'sora', label: 'Sora', desc: 'Soft rounded' },
    { id: 'jetbrains', label: 'JetBrains Mono', desc: 'Mono' },
  ];

  let config: AppConfig | null = $state(null);
  let saving = $state(false);
  let status = $state("");
  let detected = $state<string | null>(null);
  let purgeConfirmOpen = $state(false);
  let purgeStatus = $state("");

  function apply(cfg: AppConfig) {
    document.documentElement.setAttribute('data-theme', cfg.theme);
    document.documentElement.setAttribute('data-font', cfg.font);
  }

  onMount(async () => {
    config = await loadConfigNative();
    apply(config);
    try { detected = await api.findNmsInstall(config.game_path); } catch {}
  });

  async function save() {
    if (!config) return;
    saving = true;
    await saveConfigNative(config);
    apply(config);
    status = "Saved — will apply everywhere";
    setTimeout(() => status = "", 2000);
    saving = false;
  }

  async function onThemeChange(v: string) {
    if (!config) return;
    config.theme = v;
    apply(config);
    await save();
  }
  async function onFontChange(v: string) {
    if (!config) return;
    config.font = v;
    apply(config);
    await save();
  }

  async function pickGamePath() {
    const picked = await open({ directory: true, title: "Select No Man's Sky folder" });
    if (typeof picked === "string" && picked && config) {
      config.game_path = picked;
      await saveConfigNative(config);
      apply(config);
      try { detected = await api.findNmsInstall(picked); } catch {}
      status = "Game path updated";
      setTimeout(() => status = "", 2000);
    }
  }
  async function clearGamePath() {
    if (!config) return;
    config.game_path = null;
    await saveConfigNative(config);
    apply(config);
    try { detected = await api.findNmsInstall(null); } catch { detected = null; }
    status = "Back to auto-detect";
    setTimeout(() => status = "", 2000);
  }

  async function handlePurge() {
    if (!config) return;
    let gameRoot = config.game_path || await api.findNmsInstall(null);
    if (!gameRoot) {
      purgeStatus = "Game not found";
      setTimeout(() => purgeStatus = "", 3000);
      return;
    }
    const modsDir = await api.getModsDir(gameRoot);
    await api.deployMods(modsDir, [], config.deploy_mode || "auto");
    purgeStatus = "Purged deployed mods";
    setTimeout(() => purgeStatus = "", 3000);
    purgeConfirmOpen = false;
  }
</script>

<div class="flex-1 bg-background text-foreground">
  <header class="sticky top-0 z-10 flex h-14 items-center justify-between border-b bg-card px-5">
    <div class="flex items-center gap-3">
      <Button variant="ghost" size="sm" onclick={() => goto('/')}>← Back</Button>
      <div class="h-4 w-px bg-border"></div>
      <div class="text-sm font-semibold">Settings</div>
      <span class="hidden sm:inline text-xs text-muted-foreground">Customize NoMansManager</span>
    </div>
    <div class="text-xs text-muted-foreground">{status}</div>
  </header>

  <div class="mx-auto max-w-[720px] p-6 space-y-6">
    <Card>
      <CardHeader>
        <CardTitle class="text-sm">Game</CardTitle>
        <CardDescription>Change your No Man's Sky install path. Auto-detect uses Steam libraries.</CardDescription>
      </CardHeader>
      <CardContent class="space-y-3">
        <div class="space-y-1.5">
          <Label>Current path</Label>
          <div class="rounded-md border bg-muted/30 px-3 py-2 font-mono text-xs break-all min-h-8 flex items-center">
            {config?.game_path ?? "Auto-detect (not set)"}
          </div>
          {#if detected}
            <div class="text-xs text-muted-foreground">Detected: <span class="font-mono">{detected}</span></div>
          {/if}
        </div>
        <div class="flex gap-2">
          <Button onclick={pickGamePath}>Choose folder…</Button>
          <Button variant="outline" onclick={clearGamePath} disabled={!config?.game_path}>Use auto-detect</Button>
        </div>
        <p class="text-xs text-muted-foreground">Pick the folder that contains <span class="font-mono">GAMEDATA</span> — e.g. <span class="font-mono">.../No Man's Sky</span></p>
      </CardContent>
    </Card>

    <div>
      <h1 class="text-lg font-semibold tracking-tight">Appearance</h1>
      <p class="text-sm text-muted-foreground">Themes and fonts apply instantly and sync via Tauri store. More options coming later.</p>
    </div>

    <Card>
      <CardHeader>
        <CardTitle class="text-sm">Theme</CardTitle>
        <CardDescription>Pick your vibe — Rose Pine & Catppuccin flavors included.</CardDescription>
      </CardHeader>
      <CardContent class="space-y-4">
        <div class="space-y-2">
          <Label>Theme</Label>
          {#if config}
            <Select.Root type="single" value={config.theme} onValueChange={onThemeChange}>
              <Select.Trigger class="w-full">
                <Select.Value placeholder="Select theme" />
              </Select.Trigger>
              <Select.Content>
                {#each themes as t}
                  <Select.Item value={t.id}>
                    <div class="flex flex-col items-start">
                      <span class="text-sm">{t.label}</span>
                      <span class="text-[11px] text-muted-foreground">{t.desc}</span>
                    </div>
                  </Select.Item>
                {/each}
              </Select.Content>
            </Select.Root>
          {/if}
        </div>

        <div class="grid grid-cols-2 sm:grid-cols-4 gap-2">
          {#each themes as t}
            <button
              class="rounded-lg border p-3 text-left hover:bg-muted transition {config?.theme === t.id ? 'border-primary ring-1 ring-primary' : 'border-border'}"
              onclick={() => onThemeChange(t.id)}
            >
              <div class="text-xs font-medium">{t.label}</div>
              <div class="mt-2 flex gap-1">
                <span class="h-3 w-6 rounded" style="background: var(--color-nms-bg)"></span>
                <span class="h-3 w-6 rounded" style="background: var(--color-primary)"></span>
                <span class="h-3 w-6 rounded" style="background: var(--color-nms-panel)"></span>
              </div>
            </button>
          {/each}
        </div>

        <Separator />

        <div class="space-y-2">
          <Label>Font</Label>
          {#if config}
            <Select.Root type="single" value={config.font} onValueChange={onFontChange}>
              <Select.Trigger class="w-full">
                <Select.Value placeholder="Select font" />
              </Select.Trigger>
              <Select.Content>
                {#each fonts as f}
                  <Select.Item value={f.id}>
                    <div class="flex flex-col items-start">
                      <span class="text-sm" style="font-family: var(--font-sans)">{f.label}</span>
                      <span class="text-[11px] text-muted-foreground">{f.desc}</span>
                    </div>
                  </Select.Item>
                {/each}
              </Select.Content>
            </Select.Root>
          {/if}
          <div class="rounded-md border bg-muted/30 p-3">
            <div class="text-sm font-medium" style="font-family: var(--font-sans)">Preview — Aa Bb Cc 123</div>
            <div class="text-xs text-muted-foreground mt-1" style="font-family: var(--font-sans)">The quick brown fox jumps over the lazy dog.</div>
          </div>
        </div>
      </CardContent>
    </Card>

    <Card class="border-destructive/50 bg-destructive/5">
      <CardHeader>
        <CardTitle class="text-sm text-destructive">Danger Zone</CardTitle>
        <CardDescription>Purge deployed mods — removes all mods from the game folder.</CardDescription>
      </CardHeader>
      <CardContent class="space-y-3">
        <p class="text-xs text-muted-foreground">
          This will clear all deployed mods from <span class="font-mono">MODS</span> and restore the game to vanilla. This action cannot be undone.
        </p>
        {#if purgeStatus}
          <div class="text-xs text-muted-foreground">{purgeStatus}</div>
        {/if}
        <Button variant="destructive" size="sm" onclick={() => (purgeConfirmOpen = true)}>Purge deployed mods</Button>

        <Dialog.Root bind:open={purgeConfirmOpen}>
          <Dialog.Content class="sm:max-w-md">
            <Dialog.Header>
              <Dialog.Title>Purge deployed mods?</Dialog.Title>
              <Dialog.Description>
                This will remove all mods from your No Man's Sky install and restore vanilla files. This action cannot be undone.
              </Dialog.Description>
            </Dialog.Header>
            <Dialog.Footer>
              <Button variant="ghost" onclick={() => (purgeConfirmOpen = false)}>Cancel</Button>
              <Button variant="destructive" onclick={handlePurge}>Purge</Button>
            </Dialog.Footer>
          </Dialog.Content>
        </Dialog.Root>
      </CardContent>
    </Card>

    <div class="flex justify-end gap-2">
      <Button variant="outline" onclick={() => goto('/')}>Done</Button>
      <Button onclick={save} disabled={saving}>{saving ? "Saving…" : "Save"}</Button>
    </div>
  </div>
</div>
