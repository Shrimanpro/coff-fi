<script lang="ts">
  import { time } from "../lib/format";
  import { api } from "../lib/ipc";
  import { player, queue, toast, trackMenu, ui } from "../lib/state.svelte";
  import type { SavedQueue, Track } from "../lib/types";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";
  import VirtualList from "./VirtualList.svelte";

  let body = $state<HTMLDivElement | null>(null);
  let switcher = $state(false);
  let saving = $state(false);
  let name = $state("");
  let saved = $state<SavedQueue[]>([]);
  let dragFrom = $state<number | null>(null);
  let dragOver = $state<number | null>(null);

  async function openSwitcher() {
    switcher = !switcher;
    saving = false;
    if (switcher) saved = await api.listQueues();
  }

  async function save() {
    const n = name.trim();
    if (!n) return;
    try {
      await api.saveQueue(n);
      toast(`Saved queue “${n}”`);
      saving = false;
      name = "";
    } catch (e) {
      toast(String(e));
    }
  }

  async function remove(q: SavedQueue) {
    await api.deleteQueue(q.id);
    saved = saved.filter((x) => x.id !== q.id);
  }

  function drop(to: number) {
    if (dragFrom !== null && dragFrom !== to) api.queueEdit("move", dragFrom, to);
    dragFrom = dragOver = null;
  }

  // Bring the playing track into view when the panel opens or the track changes.
  // `player.status` is replaced ~8×/s, so depend on the derived id (which only
  // notifies on a real change) or every status tick would yank the scroll back.
  const playingId = $derived(player.status.track_id);
  $effect(() => {
    const pos = queue.pos;
    playingId;
    if (body) body.scrollTop = Math.max(0, pos * 54 - 120);
  });

  const items = $derived(queue.tracks.map((t, i) => ({ t, i })));
</script>

<div class="head">
  <div class="title-wrap">
    <span class="eyebrow">Playing from</span>
    <h3 class="ellipsis" title={queue.name}>{queue.name || "Nothing yet"}</h3>
  </div>
  <button class="icon-btn" class:on={switcher} title="Switch queue" onclick={openSwitcher}><Icon name="history" size={17} /></button>
  <button class="icon-btn" class:on={saving} title="Save queue" onclick={() => ((saving = !saving), (switcher = false))}><Icon name="save" size={16} /></button>
  {#if ui.leftPinned}
    <button class="icon-btn" title="Unpin" onclick={() => (ui.leftPinned = false)}><Icon name="close" size={15} /></button>
  {/if}
</div>

{#if saving}
  <form class="save" onsubmit={(e) => (e.preventDefault(), save())}>
    <!-- svelte-ignore a11y_autofocus -->
    <input placeholder="Name this queue" bind:value={name} autofocus />
    <button class="pill-btn" type="submit">Save</button>
  </form>
{/if}

{#if switcher}
  <div class="switcher">
    {#if saved.length === 0}
      <p class="hint">No saved or recent queues yet.</p>
    {/if}
    {#each saved as q (q.id)}
      <div class="sq" role="button" tabindex="0" onclick={() => ((switcher = false), api.loadQueue(q.id))} onkeydown={(e) => e.key === "Enter" && api.loadQueue(q.id)}>
        <div class="sq-art"><Cover covers={q.covers} size={96} radius={6} /></div>
        <div class="sq-meta">
          <div class="ellipsis">{q.name}</div>
          <div class="sub">{q.kind === "saved" ? "Saved" : "Recent"} · {q.count} songs</div>
        </div>
        {#if q.kind === "saved"}
          <button class="icon-btn" title="Delete" onclick={(e) => (e.stopPropagation(), remove(q))}><Icon name="trash" size={15} /></button>
        {/if}
      </div>
    {/each}
  </div>
{:else}
  <div class="body" bind:this={body}>
    {#if queue.tracks.length === 0}
      <p class="hint">The queue is empty. Right-click anything and choose “Add to queue”.</p>
    {/if}
    <VirtualList {items} rowHeight={54} scroller={body}>
      {#snippet row({ t, i }: { t: Track; i: number })}
        <div
          class="row"
          class:current={i === queue.pos}
          class:past={i < queue.pos}
          class:over={dragOver === i && dragFrom !== i}
          role="listitem"
          draggable="true"
          ondragstart={(e) => {
            dragFrom = i;
            e.dataTransfer?.setData("text/plain", String(i));
          }}
          ondragover={(e) => {
            e.preventDefault();
            dragOver = i;
          }}
          ondrop={(e) => {
            e.preventDefault();
            drop(i);
          }}
          ondragend={() => (dragFrom = dragOver = null)}
          ondblclick={() => api.queueEdit("jump", i)}
          oncontextmenu={(e) => trackMenu(e, t)}
        >
          <div class="art"><Cover covers={t.cover} size={96} radius={6} /></div>
          <div class="meta">
            <div class="t ellipsis">{t.title}</div>
            <div class="a ellipsis">{t.artist}</div>
          </div>
          <span class="dur">{time(t.duration)}</span>
          <button class="x icon-btn" title="Remove" onclick={() => api.queueEdit("remove", i)}><Icon name="close" size={13} /></button>
        </div>
      {/snippet}
    </VirtualList>
  </div>
{/if}

<style>
  .head {
    display: flex;
    align-items: flex-end;
    gap: 2px;
    padding: 18px 12px 12px 18px;
    border-bottom: 1px solid var(--rule);
  }
  .title-wrap {
    flex: 1;
    min-width: 0;
  }
  h3 {
    font-size: 22px;
  }
  .save {
    display: flex;
    gap: 8px;
    padding: 12px;
    border-bottom: 1px solid var(--rule);
  }
  .save input {
    flex: 1;
    min-width: 0;
    height: 36px;
    padding: 0 12px;
    border-radius: 10px;
    border: 1px solid var(--glass-border);
    background: var(--hover);
    outline: none;
  }
  .switcher,
  .body {
    flex: 1;
    overflow-y: auto;
    padding: 8px;
  }
  .sq {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px;
    border-radius: 10px;
  }
  .sq:hover {
    background: var(--hover);
  }
  .sq-art {
    width: 42px;
    flex: none;
  }
  .sq-meta {
    flex: 1;
    min-width: 0;
  }
  .sub {
    font-size: 12px;
    color: var(--text-3);
  }
  .row {
    height: 54px;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 6px;
    border-radius: 10px;
    border-top: 2px solid transparent;
  }
  .row:hover {
    background: var(--hover);
  }
  .row.current {
    background: var(--active);
  }
  .row.current .t {
    color: var(--accent);
  }
  .row.past {
    opacity: 0.55;
  }
  .row.over {
    border-top-color: var(--accent);
  }
  .art {
    width: 38px;
    flex: none;
  }
  .meta {
    flex: 1;
    min-width: 0;
  }
  .t {
    font-size: 13px;
    font-weight: 550;
  }
  .a {
    font-size: 12px;
    color: var(--text-2);
  }
  .dur {
    font-size: 11.5px;
    color: var(--text-3);
    font-variant-numeric: tabular-nums;
  }
  .x {
    width: 26px;
    height: 26px;
    display: none;
  }
  .row:hover .x {
    display: inline-grid;
  }
  .row:hover .dur {
    display: none;
  }
  .hint {
    color: var(--text-3);
    padding: 12px;
    font-size: 13px;
  }
</style>
