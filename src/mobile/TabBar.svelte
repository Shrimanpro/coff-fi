<script lang="ts">
  import { remote } from "../lib/remote.svelte";
  // Floating glass tab bar + mini player.
  // Expanded: the mini-player pill floats above the four tabs.
  // Collapsed (something is playing and you scrolled down): the tab bar shrinks to the active
  // tab only, and the mini player slides in beside it. Scrolling up, or tapping the tab,
  // expands it again.
  import { tick } from "svelte";
  import Cover from "../components/Cover.svelte";
  import Icon from "../components/Icon.svelte";
  import { haptic } from "../lib/haptics";
  import { api } from "../lib/ipc";
  import { anchors, player } from "../lib/state.svelte";
  import { m, selectTab, type TabId } from "./nav.svelte";

  const TABS: { id: TabId; label: string; icon: string }[] = [
    { id: "home", label: "Home", icon: "home" },
    { id: "library", label: "Library", icon: "library" },
    { id: "playlists", label: "Playlists", icon: "playlist" },
    { id: "search", label: "Search", icon: "search" },
  ];

  const t = $derived(player.track);
  const has = $derived(!!t);
  const collapsed = $derived(has && m.collapsed);

  let tabsEl = $state<HTMLElement>();
  let miniEl = $state<HTMLElement>();
  let artEl = $state<HTMLElement>();
  $effect(() => {
    anchors.miniArt = artEl ?? null;
  });

  // FLIP between the two layouts so the pill and the bar glide instead of jumping.
  let before: { tabs?: DOMRect; mini?: DOMRect } = {};
  $effect.pre(() => {
    collapsed;
    before = { tabs: tabsEl?.getBoundingClientRect(), mini: miniEl?.getBoundingClientRect() };
    tick().then(() => {
      const spring = "cubic-bezier(0.34, 1.26, 0.64, 1)";
      for (const [el, r] of [
        [tabsEl, before.tabs],
        [miniEl, before.mini],
      ] as const) {
        if (!el || !r || r.width === 0) continue;
        const n = el.getBoundingClientRect();
        if (n.width === 0) continue;
        const dx = r.left - n.left;
        const dy = r.top - n.top;
        const sx = r.width / n.width;
        el.animate([{ transform: `translate(${dx}px, ${dy}px) scaleX(${sx})`, transformOrigin: "left center" }, { transform: "none", transformOrigin: "left center" }], {
          duration: 420,
          easing: spring,
        });
      }
    });
  });

  function tabTap(id: TabId) {
    if (collapsed) {
      m.collapsed = false;
      return;
    }
    selectTab(id);
  }
</script>

<div class="dock" class:collapsed class:has>
  {#if t}
    <div
      class="mini glass"
      bind:this={miniEl}
      role="button"
      tabindex="0"
      aria-label="Open Now Playing"
      onclick={() => (haptic("light"), (m.nowPlaying = true))}
      onkeydown={(e) => e.key === "Enter" && (m.nowPlaying = true)}
    >
      <div class="art" bind:this={artEl}><Cover covers={t.cover} size={96} round /></div>
      <div class="text">
        <div class="title ellipsis">{t.title}</div>
        {#if remote.target}
          <div class="artist where ellipsis">On {remote.target.name}</div>
        {:else if remote.controllers.length}
          <div class="artist where ellipsis">Controlled by {remote.controllers.join(", ")}</div>
        {:else}
          <div class="artist ellipsis">{t.artist}</div>
        {/if}
      </div>
      <button class="ctl" aria-label={player.status.playing ? "Pause" : "Play"} onclick={(e) => (e.stopPropagation(), haptic("medium"), api.cmd("toggle"))}>
        <span class="pp" class:on={!player.status.playing}><Icon name="play" size={24} /></span>
        <span class="pp" class:on={player.status.playing}><Icon name="pause" size={24} /></span>
      </button>
      <button class="ctl" aria-label="Next" onclick={(e) => (e.stopPropagation(), haptic("light"), api.cmd("next"))}><Icon name="next" size={24} /></button>
    </div>
  {/if}

  <nav class="tabs glass" bind:this={tabsEl}>
    {#each TABS as tb (tb.id)}
      <button class="tab" class:on={m.tab === tb.id} class:hidden={collapsed && m.tab !== tb.id} onclick={() => tabTap(tb.id)} aria-label={tb.label}>
        <Icon name={tb.icon} size={23} />
        <span>{tb.label}</span>
      </button>
    {/each}
  </nav>
</div>

<style>
  .dock {
    position: fixed;
    left: 12px;
    right: 12px;
    bottom: max(10px, calc(env(safe-area-inset-bottom) - 8px));
    z-index: 50;
    display: flex;
    align-items: flex-end;
    gap: 10px;
  }
  .glass {
    background: color-mix(in srgb, var(--glass-strong) 82%, transparent);
    backdrop-filter: blur(24px) saturate(180%);
    -webkit-backdrop-filter: blur(24px) saturate(180%);
    border: 0.5px solid var(--glass-border);
    box-shadow: inset 0 1px 0 var(--glass-hi), 0 10px 30px rgba(0, 0, 0, 0.28);
  }

  /* ---- mini player ---- */
  .mini {
    position: absolute;
    left: 0;
    right: 0;
    bottom: calc(100% + 9px);
    height: 58px;
    border-radius: 29px;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 8px 0 9px;
    -webkit-tap-highlight-color: transparent;
  }
  .collapsed .mini {
    position: relative;
    bottom: auto;
    flex: 1;
    min-width: 0;
    height: 62px;
    border-radius: 31px;
  }
  .art {
    width: 42px;
    flex: none;
    border-radius: 50%;
    overflow: hidden;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.25);
  }
  .mini {
    transition: transform 0.45s cubic-bezier(0.3, 1.4, 0.5, 1);
  }
  .mini:active:not(:has(.ctl:active)) {
    transform: scale(0.97);
    transition-duration: 0.1s;
  }
  .ctl {
    position: relative;
    transition: transform 0.45s cubic-bezier(0.3, 1.4, 0.5, 1), background-color 0.2s;
  }
  .ctl:active {
    transform: scale(0.84);
    transition-duration: 0.08s;
  }
  .pp {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    opacity: 0;
    transform: scale(0.4) rotate(-25deg);
    transition:
      opacity 0.18s ease,
      transform 0.45s cubic-bezier(0.3, 1.4, 0.5, 1);
  }
  .pp.on {
    opacity: 1;
    transform: none;
  }
  .text {
    flex: 1;
    min-width: 0;
  }
  .title {
    font-size: 14.5px;
    font-weight: 600;
  }
  .artist {
    font-size: 12.5px;
    color: var(--text-2);
  }
  .ctl {
    width: 42px;
    height: 42px;
    flex: none;
    display: grid;
    place-items: center;
    border-radius: 50%;
    color: var(--text);
  }
  .ctl:active {
    background: var(--active);
  }

  /* ---- tabs ---- */
  .tabs {
    flex: 1;
    height: 62px;
    border-radius: 31px;
    display: flex;
    align-items: center;
    padding: 4px;
    gap: 2px;
  }
  .collapsed .tabs {
    order: -1;
    flex: none;
    width: 62px;
    padding: 4px;
  }
  .tab {
    flex: 1;
    height: 100%;
    border-radius: 27px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    color: var(--text-2);
    font-size: 10.5px;
    font-weight: 600;
    -webkit-tap-highlight-color: transparent;
    transition: background-color 0.25s var(--ease), color 0.25s;
  }
  .tab.on {
    color: var(--accent-2);
    background: color-mix(in srgb, var(--accent) 16%, transparent);
  }
  :global([data-theme="light"]) .tab.on {
    color: var(--accent);
  }
  .tab.hidden {
    display: none;
  }
  .collapsed .tab span {
    display: none;
  }
  .where {
    color: var(--accent);
    font-weight: 600;
  }
</style>
