<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import { page } from '$app/state';
  import { loadConfigNative } from '$lib/config';
  import pkg from '../../package.json';
  let { children } = $props();
  const appVersion = pkg.version;

  const themes = ['default','rose-pine','rose-pine-moon','rose-pine-dawn','catppuccin-mocha','catppuccin-macchiato','catppuccin-frappe','catppuccin-latte'];
  const fonts = ['inter','jetbrains','geist','space','manrope','sora'];

  function apply(cfg: any) {
    const t = themes.includes(cfg?.theme) ? cfg.theme : 'default';
    const f = fonts.includes(cfg?.font) ? cfg.font : 'inter';
    document.documentElement.setAttribute('data-theme', t);
    document.documentElement.setAttribute('data-font', f);
  }

  onMount(() => {
    let id: ReturnType<typeof setInterval> | undefined;
    (async () => {
      try {
        const cfg = await loadConfigNative();
        apply(cfg);
        id = setInterval(async () => {
          try { const c = await loadConfigNative(); apply(c); } catch {}
        }, 1000);
      } catch {}
    })();
    return () => { if (id) clearInterval(id); };
  });

  let path = $derived(page.url.pathname);
  const isActive = (href: string) => href === '/' ? path === '/' : path.startsWith(href);

  let collapsed = $state(false);
  onMount(() => {
    try {
      const saved = localStorage.getItem("sidebar-collapsed");
      if (saved !== null) collapsed = saved === "true";
    } catch {}
  });
  function toggleCollapsed() {
    collapsed = !collapsed;
    try { localStorage.setItem("sidebar-collapsed", String(collapsed)); } catch {}
  }
</script>

<svelte:head>
  <title>NoMansManager — NMS Mod Manager</title>
</svelte:head>

<div class="flex h-screen bg-background text-foreground overflow-hidden">
  <aside class="shrink-0 flex flex-col bg-card border-r transition-all duration-200 {collapsed ? 'w-[56px] items-center' : 'w-[220px]'}">
    <div class="h-12 flex items-center gap-2 px-3 border-b shrink-0 w-full {collapsed ? 'justify-center' : ''}">
      <div class="h-7 w-7 rounded-md bg-primary flex items-center justify-center text-primary-foreground font-black text-[11px] shrink-0">NMM</div>
      {#if !collapsed}
        <div class="leading-tight min-w-0">
          <div class="text-sm font-semibold tracking-tight truncate">NoMansManager <span class="text-[11px] font-normal text-muted-foreground">v{appVersion}</span></div>
          <div class="text-[11px] text-muted-foreground">Mods & Bases</div>
        </div>
      {/if}
    </div>
    <nav class="flex-1 flex flex-col gap-1 w-full p-1.5 overflow-y-auto">
      <a href="/" title="Mods" class="flex items-center gap-2.5 rounded-md px-2.5 py-2 text-sm transition {isActive('/') ? 'bg-primary text-primary-foreground' : 'text-muted-foreground hover:bg-muted hover:text-foreground'} {collapsed ? 'justify-center px-1' : ''}">
        <span class="text-sm leading-none shrink-0">◫</span>
        {#if !collapsed}<span>Mods</span>{/if}
      </a>
      <a href="/profiles" title="Profiles" class="flex items-center gap-2.5 rounded-md px-2.5 py-2 text-sm transition {isActive('/profiles') ? 'bg-primary text-primary-foreground' : 'text-muted-foreground hover:bg-muted hover:text-foreground'} {collapsed ? 'justify-center px-1' : ''}">
        <span class="text-sm leading-none">👥</span>
        {#if !collapsed}<span>Profiles</span>{/if}
      </a>
      <a href="/bases" title="Bases" class="flex items-center gap-2.5 rounded-md px-2.5 py-2 text-sm transition {isActive('/bases') ? 'bg-primary text-primary-foreground' : 'text-muted-foreground hover:bg-muted hover:text-foreground'} {collapsed ? 'justify-center px-1' : ''}">
        <span class="text-sm leading-none shrink-0">🛰</span>
        {#if !collapsed}<span>Bases</span>{/if}
      </a>
      <a href="/settings" title="Settings" class="flex items-center gap-2.5 rounded-md px-2.5 py-2 text-sm transition {isActive('/settings') ? 'bg-primary text-primary-foreground' : 'text-muted-foreground hover:bg-muted hover:text-foreground'} {collapsed ? 'justify-center px-1' : ''}">
        <span class="text-sm leading-none">⚙</span>
        {#if !collapsed}<span>Settings</span>{/if}
      </a>
    </nav>
    <div class="p-2 w-full">
      <button
        class="h-7 w-full rounded-md border bg-background hover:bg-muted flex items-center justify-center text-xs text-muted-foreground hover:text-foreground transition"
        onclick={toggleCollapsed}
        aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
        title={collapsed ? "Expand" : "Collapse"}
      >
        <span class="text-xs">{collapsed ? "›" : "‹"}</span>
        {#if !collapsed}<span class="ml-1.5 text-xs">Collapse</span>{/if}
      </button>
    </div>
  </aside>

  <div class="flex flex-1 flex-col min-w-0 overflow-auto bg-background">
    {@render children()}
  </div>
</div>
