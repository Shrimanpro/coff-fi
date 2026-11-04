<script lang="ts">
  // All playlists at a glance: smart playlists, Navidrome playlists, saved and recent queues.
  import Cover from "../../components/Cover.svelte";
  import Icon from "../../components/Icon.svelte";
  import { api } from "../../lib/ipc";
  import { toast, ui } from "../../lib/state.svelte";
  import type { Home, SavedQueue } from "../../lib/types";
  import MTile from "../MTile.svelte";
  import { longpress, openSheet } from "../nav.svelte";
  import PageHead from "../PageHead.svelte";

  let home = $state<Home | null>(null);
  let queues = $state<SavedQueue[]>([]);

  const load = () => {
    api.home().then((h) => (home = h)).catch((e) => toast(String(e)));
    api.listQueues().then((q) => (queues = q)).catch(() => {});
  };
  $effect(() => {
    ui.libVersion;
    ui.statsVersion;
    load();
  });

  const saved = $derived(queues.filter((q) => q.kind === "saved"));
  const recent = $derived(queues.filter((q) => q.kind === "history"));

  function play(q: SavedQueue) {
    api.loadQueue(q.id).then(() => toast(`Playing “${q.name}”`)).catch((e) => toast(String(e)));
  }
  function queueSheet(q: SavedQueue) {
    openSheet(
      [
        { label: "Play", icon: "play", action: () => play(q) },
        {
          label: "Delete",
          icon: "trash",
          danger: true,
          action: async () => {
            await api.deleteQueue(q.id);
            queues = queues.filter((x) => x.id !== q.id);
          },
        },
      ],
      q.name,
      `${q.count} songs`,
    );
  }
  const ago = (ts: number) => {
    const s = Date.now() / 1000 - ts;
    return s < 3600 ? `${Math.max(1, Math.round(s / 60))} min ago` : s < 86400 ? `${Math.round(s / 3600)} h ago` : new Date(ts * 1000).toLocaleDateString();
  };
</script>

<PageHead title="Playlists" />

{#if home}
  <section class="grid">
    {#each home.server_playlists as t (t.id)}<MTile tile={t} />{/each}
    {#each home.playlists as t (t.id)}<MTile tile={t} />{/each}
  </section>
{/if}

{#snippet qlist(list: SavedQueue[])}
  {#each list as q (q.id)}
    <div class="qrow" role="button" tabindex="0" onclick={() => play(q)} onkeydown={(e) => e.key === "Enter" && play(q)} use:longpress={() => queueSheet(q)}>
      <div class="qart"><Cover covers={q.covers} size={96} radius={8} /></div>
      <div class="qtext">
        <div class="ellipsis">{q.name}</div>
        <div class="qsub">{q.count} songs · {ago(q.updated_at)}</div>
      </div>
      <button class="more" aria-label="More" onclick={(e) => (e.stopPropagation(), queueSheet(q))}><Icon name="more" size={18} /></button>
    </div>
  {/each}
{/snippet}

{#if saved.length}
  <h2>Saved queues</h2>
  {@render qlist(saved)}
{/if}
{#if recent.length}
  <h2>Recent queues</h2>
  {@render qlist(recent)}
{/if}

<style>
  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 18px 14px;
    padding: 0 18px 10px;
  }
  h2 {
    font-size: 24px;
    padding: 18px 18px 8px;
  }
  .qrow {
    display: flex;
    align-items: center;
    gap: 14px;
    height: 64px;
    padding: 0 6px 0 18px;
    -webkit-tap-highlight-color: transparent;
  }
  .qrow:active {
    background: var(--hover);
  }
  .qart {
    width: 48px;
    flex: none;
    border-radius: 8px;
    overflow: hidden;
  }
  .qtext {
    flex: 1;
    min-width: 0;
    font-size: 15.5px;
  }
  .qsub {
    font-size: 13px;
    color: var(--text-2);
  }
  .more {
    width: 40px;
    height: 44px;
    display: grid;
    place-items: center;
    color: var(--text-2);
  }
</style>
