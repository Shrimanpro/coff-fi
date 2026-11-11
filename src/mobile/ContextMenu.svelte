<script lang="ts">
  // iOS-style context menu (long-press): the backdrop blurs, the pressed item lifts out at its
  // own position (nudged up if the menu needs room) and the menu springs open beside it.
  import Cover from "../components/Cover.svelte";
  import Icon from "../components/Icon.svelte";
  import { haptic } from "../lib/haptics";
  import { m, type ContextMenu } from "./nav.svelte";

  const ROW_H = 48;
  const MENU_W = 236;
  const GAP = 10;
  const SPRING = "cubic-bezier(0.32, 1.25, 0.55, 1)";

  let shown = $state<ContextMenu | null>(null);
  let open = $state(false);
  let closing = false;

  $effect(() => {
    const c = m.ctx;
    if (c && c !== shown) {
      shown = c;
      closing = false;
      open = false;
      // Next frame: flip to the open state so the CSS transitions run on the compositor.
      requestAnimationFrame(() => requestAnimationFrame(() => (open = true)));
    }
  });

  const layout = $derived.by(() => {
    if (!shown) return null;
    const r = shown.rect;
    const vh = window.innerHeight;
    const vw = window.innerWidth;
    const safeTop = 54;
    const safeBottom = 34;
    // Items plus the title header (tiles only), so the whole menu always fits on screen.
    const menuH = shown.items.length * ROW_H + (shown.preview.row ? 0 : 52);
    // Prefer the menu below the item; if it doesn't fit, lift the item up (or put the menu above).
    let shift = 0;
    let below = true;
    const room = vh - safeBottom - (r.bottom + GAP + menuH);
    if (room < 0) {
      const canLift = r.top - safeTop;
      if (canLift >= -room) shift = room;
      else below = false;
    }
    const top = r.top + shift;
    const menuTop = below ? top + r.height + GAP : Math.max(safeTop, top - GAP - menuH);
    const alignRight = r.left + r.width / 2 > vw / 2;
    const menuLeft = Math.min(Math.max(12, alignRight ? r.right - MENU_W : r.left), vw - MENU_W - 12);
    return { top, left: r.left, width: r.width, height: r.height, shift, menuTop, menuLeft, below, alignRight };
  });

  function close(then?: () => void) {
    if (closing) return;
    closing = true;
    open = false;
    setTimeout(() => {
      m.ctx = null;
      shown = null;
      then?.();
    }, 260);
  }

  function pick(action: () => void) {
    haptic("selection");
    close();
    // Run right away so playback starts while the menu fades.
    action();
  }
</script>

{#if shown && layout}
  {@const L = layout}
  <div class="ctx" class:open role="presentation">
    <button class="backdrop" aria-label="Close menu" onclick={() => close()}></button>

    <div
      class="preview"
      class:row={shown.preview.row}
      class:round={shown.preview.round}
      style:left="{L.left}px"
      style:top="{L.top - L.shift}px"
      style:width="{L.width}px"
      style:height="{L.height}px"
      style:--shift="{L.shift}px"
    >
      {#if shown.preview.row}
        <div class="rart"><Cover covers={shown.preview.covers} size={96} radius={7} /></div>
        <div class="rtext">
          <div class="rt ellipsis">{shown.preview.title}</div>
          {#if shown.preview.subtitle}<div class="rs ellipsis">{shown.preview.subtitle}</div>{/if}
        </div>
      {:else}
        <Cover covers={shown.preview.covers} size={640} radius={shown.preview.round ? 0 : 10} round={shown.preview.round} />
      {/if}
    </div>

    <div
      class="menu"
      role="menu"
      style:left="{L.menuLeft}px"
      style:top="{L.menuTop}px"
      style:width="{MENU_W}px"
      style:transform-origin="{L.alignRight ? 'right' : 'left'} {L.below ? 'top' : 'bottom'}"
      style:--spring={SPRING}
    >
      {#if !shown.preview.row}
        <div class="mhead">
          <div class="mt ellipsis">{shown.preview.title}</div>
          {#if shown.preview.subtitle}<div class="ms ellipsis">{shown.preview.subtitle}</div>{/if}
        </div>
      {/if}
      {#each shown.items as it, i (it.label)}
        <button class="item" class:danger={it.danger} role="menuitem" style:--i={i} onclick={() => pick(it.action)}>
          <span>{it.label}</span>
          {#if it.icon}<Icon name={it.icon} size={18} />{/if}
        </button>
      {/each}
    </div>
  </div>
{/if}

<style>
  .ctx {
    position: fixed;
    inset: 0;
    z-index: 190;
  }
  .backdrop {
    position: absolute;
    inset: 0;
    width: 100%;
    background: rgba(0, 0, 0, 0.28);
    backdrop-filter: blur(18px) saturate(140%);
    -webkit-backdrop-filter: blur(18px) saturate(140%);
    opacity: 0;
    transition: opacity 0.26s ease;
  }
  .open .backdrop {
    opacity: 1;
  }

  .preview {
    position: absolute;
    border-radius: 10px;
    overflow: hidden;
    /* Starts exactly where the pressed item was (already sunk to 0.95), then lifts. */
    transform: translateY(0) scale(0.95);
    box-shadow: 0 0 0 rgba(0, 0, 0, 0);
    transition:
      transform 0.5s cubic-bezier(0.32, 1.25, 0.55, 1),
      box-shadow 0.4s ease;
    will-change: transform;
  }
  .open .preview {
    transform: translateY(var(--shift)) scale(1.03);
    box-shadow: 0 22px 50px rgba(0, 0, 0, 0.35);
  }
  .preview.round {
    border-radius: 50%;
  }
  .preview.row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 16px;
    border-radius: 14px;
    background: var(--glass-strong);
  }
  .rart {
    width: 46px;
    flex: none;
    border-radius: 7px;
    overflow: hidden;
  }
  .rtext {
    min-width: 0;
  }
  .rt {
    font-size: 15.5px;
    font-weight: 600;
  }
  .rs {
    font-size: 13px;
    color: var(--text-2);
  }

  .menu {
    position: absolute;
    border-radius: 16px;
    overflow: hidden;
    background: color-mix(in srgb, var(--glass-strong) 92%, transparent);
    backdrop-filter: blur(30px) saturate(180%);
    -webkit-backdrop-filter: blur(30px) saturate(180%);
    border: 0.5px solid var(--glass-border);
    box-shadow: 0 18px 50px rgba(0, 0, 0, 0.3);
    opacity: 0;
    transform: scale(0.4);
    transition:
      transform 0.48s var(--spring),
      opacity 0.2s ease;
    will-change: transform;
  }
  .open .menu {
    opacity: 1;
    transform: none;
  }
  .mhead {
    padding: 10px 16px 8px;
    border-bottom: 0.5px solid var(--rule);
  }
  .mt {
    font-size: 13px;
    font-weight: 600;
  }
  .ms {
    font-size: 12px;
    color: var(--text-2);
  }
  .item {
    width: 100%;
    height: 48px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 16px;
    font-size: 16px;
    color: var(--text);
    text-align: left;
    -webkit-tap-highlight-color: transparent;
    transition: background-color 0.15s;
  }
  .item + .item {
    border-top: 0.5px solid var(--rule);
  }
  .item:active {
    background: var(--active);
  }
  .item :global(svg) {
    color: var(--text-2);
  }
  .danger,
  .danger :global(svg) {
    color: #e0605a !important;
  }
</style>
