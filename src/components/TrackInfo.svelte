<script lang="ts">
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { fade, scale } from "svelte/transition";
  import { quality, time } from "../lib/format";
  import { api } from "../lib/ipc";
  import { player, toast, ui } from "../lib/state.svelte";
  import type { FileInfo, Tags } from "../lib/types";
  import Cover from "./Cover.svelte";
  import Icon from "./Icon.svelte";


  // Opened only for local songs; the panel snapshots the track it was opened for.
  const t = ui.info!;
  let info = $state<FileInfo | null>(null);
  let tags = $state<Tags | null>(null);
  let original = "";
  let saving = $state(false);
  let failed = $state<string | null>(null);

  api
    .getTags(t.id)
    .then((i) => {
      info = i;
      tags = { ...i.tags };
      original = JSON.stringify(i.tags);
    })
    .catch((e) => (failed = String(e)));

  const dirty = $derived(!!tags && JSON.stringify(tags) !== original);

  function num(v: string): number | null {
    const n = parseInt(v, 10);
    return n > 0 ? n : null;
  }

  function size(b: number) {
    return b >= 1 << 20 ? `${(b / (1 << 20)).toFixed(1)} MB` : `${Math.max(1, Math.round(b / 1024))} KB`;
  }

  async function save() {
    if (!tags || !dirty || saving) return;
    saving = true;
    try {
      const updated = await api.setTags(t.id, $state.snapshot(tags));
      if (player.track?.id === t.id) player.track = updated;
      toast("Song info saved");
      close();
    } catch (e) {
      toast(String(e));
    }
    saving = false;
  }

  function close() {
    ui.info = null;
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && !ui.menu && close()} />

<div class="scrim" transition:fade={{ duration: 200 }} role="presentation" onclick={close}></div>
<div class="dialog glass" transition:scale={{ start: 0.96, duration: 260 }} role="dialog" aria-label="Song info">
  <header>
    <div class="art"><Cover covers={t.cover} size={192} radius={12} /></div>
    <div class="head-meta">
      <span class="eyebrow">Song info</span>
      <h2 class="ellipsis" title={t.title}>{t.title}</h2>
      <div class="sub ellipsis">{t.artist}</div>
    </div>
    <button class="icon-btn x" title="Close" onclick={close}><Icon name="close" /></button>
  </header>

  {#if failed}
    <p class="msg bad">{failed}</p>
  {:else if !tags || !info}
    <p class="msg">Reading tags…</p>
  {:else}
    <form class="fields" onsubmit={(e) => (e.preventDefault(), save())}>
      <label class="wide"><span>Title</span><input bind:value={tags.title} disabled={info.read_only} /></label>
      <label><span>Artist</span><input bind:value={tags.artist} disabled={info.read_only} /></label>
      <label><span>Album artist</span><input bind:value={tags.album_artist} placeholder={tags.artist} disabled={info.read_only} /></label>
      <label><span>Album</span><input bind:value={tags.album} disabled={info.read_only} /></label>
      <label><span>Genre</span><input bind:value={tags.genre} disabled={info.read_only} /></label>
      <div class="nums">
        <label><span>Year</span><input inputmode="numeric" value={tags.year ?? ""} oninput={(e) => (tags!.year = num(e.currentTarget.value))} disabled={info.read_only} /></label>
        <label>
          <span>Track</span>
          <div class="of">
            <input inputmode="numeric" value={tags.track_no ?? ""} oninput={(e) => (tags!.track_no = num(e.currentTarget.value))} disabled={info.read_only} />
            <em>of</em>
            <input inputmode="numeric" value={tags.track_total ?? ""} oninput={(e) => (tags!.track_total = num(e.currentTarget.value))} disabled={info.read_only} />
          </div>
        </label>
        <label>
          <span>Disc</span>
          <div class="of">
            <input inputmode="numeric" value={tags.disc_no ?? ""} oninput={(e) => (tags!.disc_no = num(e.currentTarget.value))} disabled={info.read_only} />
            <em>of</em>
            <input inputmode="numeric" value={tags.disc_total ?? ""} oninput={(e) => (tags!.disc_total = num(e.currentTarget.value))} disabled={info.read_only} />
          </div>
        </label>
      </div>

      <dl class="file wide">
        <div><dt>Format</dt><dd>{quality(t)}</dd></div>
        <div><dt>Length</dt><dd>{time(t.duration)}</dd></div>
        <div><dt>Size</dt><dd>{size(info.size)}</dd></div>
        <div class="path">
          <dt>File</dt>
          <dd class="ellipsis" title={t.path}>{t.path}</dd>
          <button type="button" class="icon-btn" title="Show file" onclick={() => revealItemInDir(t.path).catch((x) => toast(String(x)))}><Icon name="folder" size={15} /></button>
        </div>
      </dl>

      <footer class="wide">
        {#if info.read_only}<span class="note">This file is read-only, so its tags can't be changed.</span>{:else}<span class="note">Changes are written into the file itself.</span>{/if}
        <button type="button" class="pill-btn ghost" onclick={close}>Cancel</button>
        <button type="submit" class="pill-btn" disabled={!dirty || saving || info.read_only}>{saving ? "Saving…" : "Save"}</button>
      </footer>
    </form>
  {/if}
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 80;
    background: rgba(10, 6, 3, 0.35);
  }
  .dialog {
    position: fixed;
    z-index: 81;
    left: 50%;
    top: 50%;
    translate: -50% -50%;
    width: min(620px, calc(100vw - 48px));
    max-height: calc(100vh - 80px);
    overflow-y: auto;
    border-radius: 26px;
    background: var(--glass-strong);
    padding: 24px 26px 22px;
  }
  .dialog.ios {
    left: 0;
    top: 0;
    translate: none;
    width: 100vw;
    height: 100%;
    max-height: none;
    border-radius: 0;
    border: 0;
    background: var(--bg);
    padding: calc(env(safe-area-inset-top) + 14px) 18px calc(env(safe-area-inset-bottom) + 24px);
  }
  .ios :global(input) {
    font-size: 16px;
  }
  header {
    display: flex;
    align-items: center;
    gap: 16px;
    padding-bottom: 18px;
    border-bottom: 1px solid var(--rule);
  }
  .art {
    width: 72px;
    flex: none;
  }
  .head-meta {
    flex: 1;
    min-width: 0;
  }
  h2 {
    font-size: 26px;
    line-height: 1.15;
  }
  .sub {
    color: var(--text-2);
    font-size: 13.5px;
  }
  .x {
    align-self: flex-start;
  }
  .msg {
    color: var(--text-3);
    padding: 24px 0 8px;
  }
  .bad {
    color: #d0675a;
  }
  .fields {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px 14px;
    padding-top: 18px;
  }
  .wide,
  .nums {
    grid-column: 1 / -1;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 5px;
    min-width: 0;
  }
  label > span {
    font-size: 11px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-3);
    font-weight: 600;
  }
  input {
    height: 36px;
    width: 100%;
    min-width: 0;
    padding: 0 12px;
    border-radius: 10px;
    border: 1px solid var(--glass-border);
    background: var(--hover);
    outline: none;
    transition: border-color 0.2s;
  }
  input:focus {
    border-color: var(--accent);
  }
  input:disabled {
    opacity: 0.6;
  }
  .nums {
    display: grid;
    grid-template-columns: 0.8fr 1fr 1fr;
    gap: 14px;
  }
  .of {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .of em {
    font-family: var(--font-serif);
    color: var(--text-3);
  }
  .file {
    display: grid;
    grid-template-columns: repeat(3, auto) 1fr;
    gap: 6px 22px;
    margin: 6px 0 0;
    padding-top: 14px;
    border-top: 1px solid var(--rule);
    font-size: 12.5px;
  }
  .file div {
    min-width: 0;
  }
  .file .path {
    grid-column: 1 / -1;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .path dd {
    flex: 1;
    min-width: 0;
  }
  dt {
    font-size: 11px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-3);
    font-weight: 600;
  }
  dd {
    margin: 2px 0 0;
    color: var(--text-2);
    font-variant-numeric: tabular-nums;
  }
  .path dd {
    margin: 0;
  }
  footer {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 8px;
  }
  .note {
    flex: 1;
    font-size: 12px;
    color: var(--text-3);
  }
  button:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
