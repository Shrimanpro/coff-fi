<script lang="ts">
  import { untrack } from "svelte";
  import { coverSrc } from "../lib/ipc";
  import { ui } from "../lib/state.svelte";
  import Icon from "./Icon.svelte";

  let {
    covers,
    size = 256,
    radius = 10,
    round = false,
    icon = "music",
  }: { covers: (string | null)[] | string | null | undefined; size?: number; radius?: number; round?: boolean; icon?: string } = $props();

  const base = $derived(coverSrc(covers, size));
  let loaded = $state(false);
  let failed = $state(false);
  // Covers can 404 while the first library sync is still filling the covers table; retry
  // failed ones when the library changes (the query is ignored by the cover protocol).
  let retry = $state(0);
  const src = $derived(base && retry ? `${base}?r=${retry}` : base);
  $effect(() => {
    base;
    loaded = false;
    failed = false;
    retry = 0;
  });
  $effect(() => {
    const v = ui.libVersion;
    if (v && untrack(() => failed)) {
      failed = false;
      retry = v;
    }
  });
</script>

<div class="cover" style:border-radius={round ? "50%" : `${radius}px`}>
  {#if src && !failed}
    <img {src} alt="" loading="lazy" decoding="async" class:loaded onload={() => (loaded = true)} onerror={() => (failed = true)} draggable="false" />
  {/if}
  {#if !src || failed || !loaded}
    <div class="ph"><Icon name={icon} size={Math.max(16, Math.min(44, size / 5))} /></div>
  {/if}
</div>

<style>
  .cover {
    position: relative;
    width: 100%;
    aspect-ratio: 1;
    overflow: hidden;
    background: var(--placeholder);
    flex: none;
  }
  img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    opacity: 0;
    transition: opacity 0.35s var(--ease);
  }
  img.loaded {
    opacity: 1;
  }
  .ph {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    color: color-mix(in srgb, var(--text) 35%, transparent);
  }
</style>
