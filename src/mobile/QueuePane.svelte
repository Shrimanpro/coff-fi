<script lang="ts">
  // The queue inside Now Playing (reference A): "Playing from …", drag handles to reorder,
  // swipe left to remove, tap to jump; plus the saved / recent queue switcher and Save queue.
  import Cover from "../components/Cover.svelte";
  import Icon from "../components/Icon.svelte";
  import { haptic } from "../lib/haptics";
  import { api } from "../lib/ipc";
  import { player, queue, toast } from "../lib/state.svelte";
  import type { SavedQueue } from "../lib/types";
  import { openSheet } from "./nav.svelte";

  const ROW = 58;
  let list = $state<HTMLDivElement>();
  let saving = $state(false);
  let name = $state("");

  // Keep the playing song at the top when the pane opens or the song changes.
  const playingId = $derived(player.status.track_id);
  $effect(() => {
    playingId;
    const pos = queue.pos;
    if (list) list.scrollTo({ top: Math.max(0, pos * ROW - 8), behavior: "smooth" });
  });

  // ---- drag to reorder (grip) ----
  let drag = $state<{ from: number; dy: number } | null>(null);
  let gy = 0;
  const over = $derived(drag ? Math.max(0, Math.min(queue.tracks.length - 1, drag.from + Math.round(drag.dy / ROW))) : -1);
  function gripDown(e: PointerEvent, i: number) {
    e.stopPropagation();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    gy = e.clientY;
    drag = { from: i, dy: 0 };
    haptic("medium");
  }
  let lastOver = -1;
  function gripMove(e: PointerEvent) {
    if (!drag) return;
    drag = { ...drag, dy: e.clientY - gy };
    // A tick for every slot the song passes, like reordering in iOS.
    if (over !== lastOver) {
      if (lastOver !== -1) haptic("selection");
      lastOver = over;
    }
  }
  function gripUp() {
    if (!drag) return;
    const { from } = drag;
    const to = over;
    drag = null;
    lastOver = -1;
    haptic("light");
    if (to !== from && to >= 0) api.queueEdit("move", from, to);
  }
  const offset = (i: number) => {
    if (!drag) return 0;
    if (i === drag.from) return drag.dy;
    if (drag.from < i && i <= over) return -ROW;
    if (over <= i && i < drag.from) return ROW;
    return 0;
  };

  // ---- swipe left to remove ----
  let sw = $state<{ i: number; dx: number } | null>(null);
  let sx = 0;
  let sy = 0;
  let axis: "x" | "y" | null = null;
  function rowDown(e: PointerEvent, i: number) {
    sx = e.clientX;
    sy = e.clientY;
    axis = null;
    sw = { i, dx: 0 };
  }
  function rowMove(e: PointerEvent) {
    if (!sw) return;
    const x = e.clientX - sx;
    const y = e.clientY - sy;
    if (!axis && (Math.abs(x) > 10 || Math.abs(y) > 10)) {
      axis = Math.abs(x) > Math.abs(y) * 1.2 && x < 0 ? "x" : "y";
      if (axis === "x") (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    }
    if (axis === "x") {
      const nx = Math.min(0, x);
      if ((nx < -90) !== (sw.dx < -90)) haptic(nx < -90 ? "medium" : "light");
      sw = { ...sw, dx: nx };
    }
  }
  function rowUp() {
    if (!sw) return;
    const { i, dx } = sw;
    const swiped = axis === "x";
    sw = null;
    if (swiped && dx < -90) {
      haptic("success");
      api.queueEdit("remove", i);
    }
    if (!swiped && axis === null) {
      haptic("light");
      api.queueEdit("jump", i);
    }
    axis = null;
  }

  async function switcher() {
    haptic("light");
    let qs: SavedQueue[] = [];
    try {
      qs = await api.listQueues();
    } catch (e) {
      toast(String(e));
      return;
    }
    if (!qs.length) {
      toast("No saved or recent queues yet");
      return;
    }
    openSheet(
      qs.slice(0, 12).map((q) => ({
        label: `${q.name} · ${q.count}`,
        icon: q.kind === "saved" ? "save" : "history",
        action: () => api.loadQueue(q.id).catch((e) => toast(String(e))),
      })),
      "Switch queue",
      "Saved queues first, then recent ones",
    );
  }

  async function save() {
    const n = name.trim();
    if (!n) return;
    try {
      await api.saveQueue(n);
      haptic("success");
      toast(`Saved queue “${n}”`);
      saving = false;
      name = "";
    } catch (e) {
      toast(String(e));
    }
  }
</script>

<div class="qhead">
  <div class="qt">
    <div class="h">Playing from</div>
    <div class="n ellipsis">{queue.name || "Queue"}</div>
  </div>
  <button class="qbtn" aria-label="Switch queue" onclick={switcher}><Icon name="history" size={18} /></button>
  <button class="qbtn" class:on={saving} aria-label="Save queue" onclick={() => (haptic("selection"), (saving = !saving))}><Icon name="save" size={17} /></button>
</div>

{#if saving}
  <form class="save" onsubmit={(e) => (e.preventDefault(), save())}>
    <!-- svelte-ignore a11y_autofocus -->
    <input placeholder="Name this queue" bind:value={name} autofocus enterkeyhint="done" />
    <button type="submit" disabled={!name.trim()}>Save</button>
  </form>
{/if}

<div class="list" bind:this={list}>
  <div class="inner" style:height="{queue.tracks.length * ROW}px">
    {#each queue.tracks as t, i (`${i}:${t.id}`)}
      <div
        class="row"
        class:past={i < queue.pos}
        class:current={i === queue.pos}
        class:lifted={drag?.from === i}
        style:top="{i * ROW}px"
        style:transform="translate({sw?.i === i ? sw.dx : 0}px, {offset(i)}px)"
        role="button"
        tabindex="0"
        onpointerdown={(e) => rowDown(e, i)}
        onpointermove={rowMove}
        onpointerup={rowUp}
        onpointercancel={() => ((sw = null), (axis = null))}
        onkeydown={(e) => e.key === "Enter" && api.queueEdit("jump", i)}
      >
        <div class="art"><Cover covers={t.cover} size={96} radius={6} /></div>
        <div class="text">
          <div class="title ellipsis">{t.title}</div>
          <div class="sub ellipsis">{t.artist}</div>
        </div>
        <span
          class="grip"
          role="presentation"
          onpointerdown={(e) => gripDown(e, i)}
          onpointermove={gripMove}
          onpointerup={gripUp}
          onpointercancel={gripUp}
        >
          <svg width="22" height="14" viewBox="0 0 22 14"><path d="M1 2h20M1 7h20M1 12h20" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" /></svg>
        </span>
      </div>
    {/each}
  </div>
</div>

<style>
  .qhead {
    display: flex;
    align-items: center;
    gap: 8px;
    padding-bottom: 8px;
  }
  .qt {
    flex: 1;
    min-width: 0;
  }
  .h {
    font-family: var(--font-serif);
    font-size: 22px;
    font-weight: 600;
  }
  .n {
    font-size: 14px;
    color: rgba(255, 255, 255, 0.65);
  }
  .qbtn {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: #fff;
    background: rgba(255, 255, 255, 0.14);
  }
  .qbtn.on {
    color: #1b130d;
    background: rgba(255, 255, 255, 0.82);
  }
  .save {
    display: flex;
    gap: 8px;
    margin-bottom: 8px;
  }
  .save input {
    flex: 1;
    min-width: 0;
    height: 38px;
    border-radius: 10px;
    border: 0;
    outline: 0;
    padding: 0 12px;
    font-size: 16px;
    color: #fff;
    background: rgba(255, 255, 255, 0.14);
    -webkit-user-select: text;
    user-select: text;
  }
  .save button {
    padding: 0 14px;
    border-radius: 10px;
    font-weight: 600;
    color: #1b130d;
    background: rgba(255, 255, 255, 0.85);
  }
  .list {
    height: calc(100% - 52px);
    overflow-y: auto;
    -webkit-mask-image: linear-gradient(#000 85%, transparent);
    mask-image: linear-gradient(#000 85%, transparent);
  }
  .inner {
    position: relative;
  }
  .row {
    position: absolute;
    left: 0;
    right: 0;
    height: 58px;
    display: flex;
    align-items: center;
    gap: 12px;
    transition: transform 0.22s var(--ease);
    -webkit-tap-highlight-color: transparent;
    touch-action: pan-y;
  }
  .row.lifted {
    transition: none;
    z-index: 2;
    background: rgba(255, 255, 255, 0.12);
    border-radius: 10px;
  }
  .row.past {
    opacity: 0.45;
  }
  .row.current .title {
    color: var(--accent-2);
  }
  .art {
    width: 44px;
    flex: none;
    border-radius: 6px;
    overflow: hidden;
  }
  .text {
    flex: 1;
    min-width: 0;
  }
  .title {
    font-size: 16px;
  }
  .sub {
    font-size: 13.5px;
    color: rgba(255, 255, 255, 0.6);
  }
  .grip {
    width: 44px;
    height: 100%;
    display: grid;
    place-items: center;
    color: rgba(255, 255, 255, 0.5);
    touch-action: none;
  }
</style>
