<script lang="ts">
  // Grouped search over the library (same results as the desktop search palette).
  import Icon from "../../components/Icon.svelte";
  import { api } from "../../lib/ipc";
  import { playTracks, ui } from "../../lib/state.svelte";
  import type { SearchResults } from "../../lib/types";
  import MSongRow from "../MSongRow.svelte";
  import MTile from "../MTile.svelte";
  import PageHead from "../PageHead.svelte";

  let q = $state("");
  let res = $state<SearchResults | null>(null);
  let input = $state<HTMLInputElement>();
  let timer: ReturnType<typeof setTimeout> | undefined;
  let seq = 0;

  $effect(() => {
    const query = q.trim();
    ui.libVersion;
    clearTimeout(timer);
    if (!query) {
      res = null;
      return;
    }
    const n = ++seq;
    timer = setTimeout(() => {
      api.search(query).then((r) => n === seq && (res = r));
    }, 140);
  });

  const empty = $derived(res && !res.songs.length && !res.albums.length && !res.artists.length && !res.playlists.length);
</script>

<PageHead title="Search" />

<div class="field">
  <Icon name="search" size={17} />
  <input bind:this={input} bind:value={q} placeholder="Songs, albums, artists, playlists" autocapitalize="off" autocorrect="off" spellcheck="false" enterkeyhint="search" onkeydown={(e) => e.key === "Enter" && input?.blur()} />
  {#if q}<button aria-label="Clear" onclick={() => ((q = ""), input?.focus())}><Icon name="close" size={15} /></button>{/if}
</div>

{#if res}
  {#if empty}
    <p class="none">No results for “{q.trim()}”</p>
  {/if}
  {#if res.playlists.length}
    <h2>Playlists</h2>
    <div class="shelf">{#each res.playlists.slice(0, 5) as t (t.id)}<MTile tile={t} width={140} />{/each}</div>
  {/if}
  {#if res.artists.length}
    <h2>Artists</h2>
    <div class="shelf">{#each res.artists.slice(0, 5) as t (t.id)}<MTile tile={t} width={104} />{/each}</div>
  {/if}
  {#if res.albums.length}
    <h2>Albums</h2>
    <div class="shelf">{#each res.albums.slice(0, 6) as t (t.id)}<MTile tile={t} width={140} />{/each}</div>
  {/if}
  {#if res.songs.length}
    {@const songs = res.songs.slice(0, 12)}
    <h2>Songs</h2>
    {#each songs as t, i (t.id)}
      <MSongRow track={t} onplay={() => playTracks(songs, i, `Search: ${q.trim()}`)} />
    {/each}
  {/if}
{/if}

<style>
  .field {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 18px 16px;
    height: 42px;
    padding: 0 12px;
    border-radius: 12px;
    background: var(--hover);
    color: var(--text-3);
  }
  input {
    flex: 1;
    min-width: 0;
    height: 100%;
    border: 0;
    outline: 0;
    background: none;
    font-size: 17px;
    color: var(--text);
    -webkit-user-select: text;
    user-select: text;
  }
  input::placeholder {
    color: var(--text-3);
  }
  .field button {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--text-3);
    color: var(--bg);
  }
  h2 {
    font-size: 23px;
    padding: 10px 18px 10px;
  }
  .shelf {
    display: flex;
    gap: 14px;
    overflow-x: auto;
    padding: 0 18px 8px;
    scrollbar-width: none;
  }
  .shelf::-webkit-scrollbar {
    display: none;
  }
  .none {
    padding: 30px 18px;
    text-align: center;
    color: var(--text-2);
  }
</style>
