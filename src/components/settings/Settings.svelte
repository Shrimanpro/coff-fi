<script lang="ts">
  import { getVersion } from "@tauri-apps/api/app";
  import { open } from "@tauri-apps/plugin-dialog";
  import { fade, scale } from "svelte/transition";
  import { api } from "../../lib/ipc";
  import { conf, saveSettings, toast, ui } from "../../lib/state.svelte";
  import Icon from "../Icon.svelte";
  import VSlider from "./VSlider.svelte";

  type Tab = "eq" | "library" | "playback" | "integrations" | "navidrome" | "devices";
  let tab = $state<Tab>("eq");
  let version = $state("");
  // "0.8.0" → "v0.8"; a patch number only shows when it isn't zero.
  getVersion().then((v) => (version = `v${v.replace(/\.0$/, "")}`));

  const s = $derived(conf.s!);
  const FREQS = ["31", "62", "125", "250", "500", "1k", "2k", "4k", "8k", "16k"];
  const PRESETS: Record<string, number[]> = {
    Flat: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    "Bass Boost": [6, 5, 4, 2, 0, 0, 0, 0, 0, 0],
    Warm: [3, 3, 2, 1, 0, -1, -1, 0, 1, 1],
    "Lo-fi": [4, 3, 1, 0, -1, -2, -3, -4, -6, -8],
    Vocal: [-2, -2, -1, 1, 3, 4, 3, 1, 0, -1],
    Acoustic: [3, 3, 2, 1, 1, 1, 2, 3, 2, 1],
    Classical: [4, 3, 2, 1, -1, -1, 0, 2, 3, 4],
    Rock: [5, 3, -1, -2, -1, 2, 4, 5, 5, 5],
    Treble: [0, 0, 0, 0, 0, 1, 2, 4, 5, 6],
  };

  // The selected preset keeps its name while you tweak; `modified` tells you the
  // curve no longer matches it. Built-ins only define bands, customs add preamp.
  const mine = $derived(s.eq_presets.find((p) => p.name === s.eq.preset));
  const builtin = $derived(PRESETS[s.eq.preset]);
  const modified = $derived(
    mine ? mine.preamp !== s.eq.preamp || mine.bands.some((b, i) => b !== s.eq.bands[i]) : builtin ? builtin.some((b, i) => b !== s.eq.bands[i]) : true,
  );
  let naming = $state<"new" | "rename" | null>(null);
  let draft = $state("");

  function setBand(i: number, v: number) {
    s.eq.bands[i] = v;
    s.eq.enabled = true;
    saveSettings();
  }

  function preset(name: string) {
    const c = s.eq_presets.find((p) => p.name === name);
    if (c) {
      s.eq.bands = [...c.bands];
      s.eq.preamp = c.preamp;
    } else if (PRESETS[name]) {
      s.eq.bands = [...PRESETS[name]];
    } else return;
    s.eq.preset = name;
    s.eq.enabled = true;
    naming = null;
    saveSettings(0);
  }

  function startNaming(kind: "new" | "rename") {
    naming = naming === kind ? null : kind;
    draft = kind === "rename" ? s.eq.preset : "";
  }

  /** Saving over a custom preset overwrites it; anything else asks for a new name. */
  function savePreset() {
    if (mine) {
      mine.bands = [...s.eq.bands];
      mine.preamp = s.eq.preamp;
      saveSettings(0);
      toast(`Saved “${mine.name}”`);
    } else startNaming("new");
  }

  function commitName() {
    const n = draft.trim();
    const taken = (x: string) => x.toLowerCase() === n.toLowerCase();
    if (!n) return;
    if (Object.keys(PRESETS).some(taken) || s.eq_presets.some((p) => p !== (naming === "rename" ? mine : null) && taken(p.name))) {
      toast(`A preset called “${n}” already exists`);
      return;
    }
    if (naming === "rename" && mine) mine.name = n;
    else s.eq_presets = [...s.eq_presets, { name: n, preamp: s.eq.preamp, bands: [...s.eq.bands] }];
    s.eq.preset = n;
    naming = null;
    saveSettings(0);
    toast(`Saved “${n}”`);
  }

  function deletePreset() {
    if (!mine) return;
    const n = mine.name;
    s.eq_presets = s.eq_presets.filter((p) => p !== mine);
    s.eq.preset = "Flat";
    s.eq.bands = [...PRESETS.Flat];
    s.eq.preamp = 0;
    naming = null;
    saveSettings(0);
    toast(`Deleted “${n}”`);
  }

  async function addFolder(list: "roots" | "excluded") {
    const dir = await open({ directory: true, multiple: false, title: list === "roots" ? "Choose a music folder" : "Choose a folder to exclude" });
    if (typeof dir === "string" && !s[list].includes(dir)) {
      s[list] = [...s[list], dir];
      saveSettings(0);
    }
  }

  function removeFolder(list: "roots" | "excluded", dir: string) {
    s[list] = s[list].filter((d) => d !== dir);
    saveSettings(0);
  }

  function close() {
    ui.settingsOpen = false;
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && !ui.menu && close()} />

<div class="scrim" transition:fade={{ duration: 200 }} role="presentation" onclick={close}></div>
<div class="dialog glass" transition:scale={{ start: 0.96, duration: 260 }} role="dialog" aria-label="Settings">
  <nav>
    <h2>Settings</h2>
    {#each [["eq", "Equalizer"], ["library", "Library"], ["playback", "Playback"]] as [id, label] (id)}
      <button class:on={tab === id} onclick={() => (tab = id as Tab)}>{label}</button>
    {/each}
    {#if version}<span class="version">Coff-fi {version}</span>{/if}
  </nav>

  <section>
    <button class="icon-btn x" title="Close" onclick={close}><Icon name="close" /></button>
    {#if s}
      {#if tab === "eq"}
        <h3>Equalizer</h3>
        <div class="line">
          <label class="switch"><input type="checkbox" bind:checked={s.eq.enabled} onchange={() => saveSettings(0)} /><span></span> Enabled</label>
          <div class="presets">
            <select value={builtin || mine ? s.eq.preset : ""} onchange={(e) => preset(e.currentTarget.value)}>
              {#if !builtin && !mine}<option value="" disabled>Custom</option>{/if}
              <optgroup label="Built-in">
                {#each Object.keys(PRESETS) as p (p)}<option value={p}>{p}</option>{/each}
              </optgroup>
              {#if s.eq_presets.length}
                <optgroup label="My presets">
                  {#each s.eq_presets as p (p.name)}<option value={p.name}>{p.name}</option>{/each}
                </optgroup>
              {/if}
            </select>
            {#if modified && (builtin || mine)}<span class="mod">Modified</span>{/if}
            <button class="icon-btn" title={mine ? `Save changes to “${mine.name}”` : "Save as a new preset"} disabled={!modified} onclick={savePreset}><Icon name="save" size={16} /></button>
            {#if mine}
              <button class="icon-btn" class:on={naming === "new"} title="Save as a new preset" onclick={() => startNaming("new")}><Icon name="plus" size={16} /></button>
              <button class="icon-btn" class:on={naming === "rename"} title="Rename preset" onclick={() => startNaming("rename")}><Icon name="pencil" size={15} /></button>
              <button class="icon-btn" title="Delete preset" onclick={deletePreset}><Icon name="trash" size={15} /></button>
            {/if}
          </div>
        </div>
        {#if naming}
          <form class="name-form" onsubmit={(e) => (e.preventDefault(), commitName())}>
            <!-- svelte-ignore a11y_autofocus -->
            <input class="text" placeholder={naming === "rename" ? "New name" : "Name this preset"} maxlength="40" bind:value={draft} autofocus onkeydown={(e) => e.key === "Escape" && (e.stopPropagation(), (naming = null))} />
            <button class="pill-btn" type="submit" disabled={!draft.trim()}>{naming === "rename" ? "Rename" : "Save"}</button>
            <button class="pill-btn ghost" type="button" onclick={() => (naming = null)}>Cancel</button>
          </form>
        {/if}
        <div class="eq" class:off={!s.eq.enabled}>
          <VSlider
            label="Pre"
            value={s.eq.preamp}
            onchange={(v) => {
              s.eq.preamp = v;
              saveSettings();
            }}
          />
          <span class="eq-sep"></span>
          {#each FREQS as f, i (f)}
            <VSlider label={f} value={s.eq.bands[i]} onchange={(v) => setBand(i, v)} />
          {/each}
        </div>
        <p class="help">Drag a band to shape the sound; double-click resets it. Changes apply instantly. Save a curve to keep it as your own preset.</p>
      {:else if tab === "library"}
        <h3>Music folders</h3>
        <ul class="dirs">
          {#each s.roots as d (d)}
            <li><Icon name="folder" size={15} /><span class="ellipsis">{d}</span><button class="icon-btn" title="Remove" onclick={() => removeFolder("roots", d)}><Icon name="close" size={14} /></button></li>
          {/each}
        </ul>
        <button class="pill-btn ghost" onclick={() => addFolder("roots")}><Icon name="plus" size={15} /> Add folder</button>

        <h3>Excluded folders</h3>
        <ul class="dirs">
          {#each s.excluded as d (d)}
            <li><Icon name="folder" size={15} /><span class="ellipsis">{d}</span><button class="icon-btn" title="Remove" onclick={() => removeFolder("excluded", d)}><Icon name="close" size={14} /></button></li>
          {:else}
            <li class="none">None</li>
          {/each}
        </ul>
        <button class="pill-btn ghost" onclick={() => addFolder("excluded")}><Icon name="plus" size={15} /> Exclude folder</button>

        <h3>Scanning</h3>
        <div class="line">
          <button class="pill-btn" disabled={ui.scanning} onclick={() => api.rescan()}><Icon name="refresh" size={15} /> {ui.scanning ? "Scanning…" : "Rescan now"}</button>
        </div>
      {:else if tab === "playback"}
        <h3>Transitions</h3>
        <label class="switch"><input type="checkbox" bind:checked={s.gapless} onchange={() => saveSettings(0)} /><span></span> Gapless playback</label>
        <div class="range">
          <span>Crossfade</span>
          <input type="range" min="0" max="12" step="0.5" bind:value={s.crossfade} oninput={() => saveSettings(300)} />
          <span class="v">{s.crossfade ? `${s.crossfade}s` : "Off"}</span>
        </div>

        <h3>Volume normalization</h3>
        <div class="seg">
          {#each [["off", "Off"], ["track", "Track"], ["album", "Album"]] as [v, l] (v)}
            <button class:on={s.replaygain === v} onclick={() => ((s.replaygain = v as typeof s.replaygain), saveSettings(0))}>{l}</button>
          {/each}
        </div>
        <div class="range">
          <span>Preamp</span>
          <input type="range" min="-12" max="12" step="0.5" bind:value={s.rg_preamp} oninput={() => saveSettings(300)} />
          <span class="v">{s.rg_preamp > 0 ? "+" : ""}{s.rg_preamp} dB</span>
        </div>
        <p class="help">Uses ReplayGain tags in your files.</p>

      {/if}
    {/if}
  </section>
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
    width: min(860px, calc(100vw - 48px));
    height: min(600px, calc(100vh - 80px));
    display: grid;
    grid-template-columns: 200px 1fr;
    border-radius: 26px;
    background: var(--glass-strong);
    overflow: hidden;
  }
  nav {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 26px 14px;
    border-right: 1px solid var(--rule);
  }
  nav h2 {
    font-size: 30px;
    padding: 0 10px 14px;
  }
  nav button {
    text-align: left;
    padding: 9px 12px;
    border-radius: 10px;
    color: var(--text-2);
    font-weight: 550;
  }
  nav button:hover {
    background: var(--hover);
  }
  .version {
    margin-top: auto;
    padding: 0 12px;
    font-size: 12px;
    color: var(--text-3);
  }
  nav button.on {
    background: var(--active);
    color: var(--text);
  }
  section {
    position: relative;
    padding: 26px 30px 30px;
    overflow-y: auto;
  }
  .x {
    position: absolute;
    top: 18px;
    right: 18px;
  }
  h3 {
    font-size: 21px;
    margin: 22px 0 12px;
  }
  h3:first-of-type {
    margin-top: 4px;
  }
  .line {
    display: flex;
    align-items: center;
    gap: 16px;
    margin: 12px 0;
  }
  select,
  .text {
    height: 36px;
    padding: 0 12px;
    border-radius: 10px;
    border: 1px solid var(--glass-border);
    background: var(--hover);
    outline: none;
    min-width: 220px;
  }
  select option {
    background: var(--bg);
  }
  .presets {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .presets select {
    margin-right: 6px;
  }
  .mod {
    font-size: 12px;
    color: var(--text-3);
    margin-right: 6px;
  }
  .name-form {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 4px 0 8px;
  }
  .name-form .text {
    flex: 1;
    max-width: 280px;
  }
  .eq {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    padding: 18px 8px 6px;
    transition: opacity 0.3s;
  }
  .eq.off {
    opacity: 0.45;
  }
  .eq-sep {
    width: 1px;
    align-self: stretch;
    background: var(--rule);
  }
  .help {
    font-size: 12.5px;
    color: var(--text-3);
    margin: 8px 0 0;
  }
  .dirs {
    list-style: none;
    padding: 0;
    margin: 0 0 10px;
  }
  .dirs li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 4px 4px 10px;
    border-radius: 10px;
    background: var(--hover);
    margin-bottom: 6px;
    color: var(--text-2);
  }
  .dirs li span {
    flex: 1;
    min-width: 0;
  }
  .dirs .none {
    background: none;
    color: var(--text-3);
    padding-left: 0;
  }
  .switch {
    display: flex;
    align-items: center;
    gap: 10px;
    cursor: pointer;
    margin: 8px 0;
  }
  .switch input {
    display: none;
  }
  .switch span {
    position: relative;
    width: 38px;
    height: 22px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--text) 18%, transparent);
    transition: background-color 0.25s;
    flex: none;
  }
  .switch span::after {
    content: "";
    position: absolute;
    top: 3px;
    left: 3px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
    transition: transform 0.3s var(--spring);
  }
  .switch input:checked + span {
    background: var(--accent);
  }
  .switch input:checked + span::after {
    transform: translateX(16px);
  }
  .range {
    display: grid;
    grid-template-columns: 80px 1fr 70px;
    align-items: center;
    gap: 12px;
    margin: 10px 0;
    color: var(--text-2);
  }
  .range input {
    accent-color: var(--accent);
  }
  .range .v {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .seg {
    display: inline-flex;
    padding: 3px;
    border-radius: 999px;
    background: var(--hover);
  }
  .seg button {
    padding: 6px 16px;
    border-radius: 999px;
    color: var(--text-2);
  }
  .seg button.on {
    background: var(--accent);
    color: var(--accent-ink);
  }
  .switch.dim {
    opacity: 0.45;
  }
  /* ---------- iOS: full-screen sheet with scrolling tabs ---------- */
  .dialog.ios {
    left: 0;
    top: 0;
    translate: none;
    width: 100vw;
    height: 100%;
    border-radius: 0;
    border: 0;
    display: flex;
    flex-direction: column;
    padding-top: env(safe-area-inset-top);
    background: var(--bg);
    backdrop-filter: none;
  }
  .ihead {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 18px 4px;
  }
  .ihead h2 {
    font-size: 32px;
  }
  .done {
    font-size: 17px;
    font-weight: 600;
    color: var(--accent-2);
  }
  :global([data-theme="light"]) .done {
    color: var(--accent);
  }
  .ios nav {
    flex: none;
    flex-direction: row;
    overflow-x: auto;
    padding: 8px 18px 12px;
    border-right: 0;
    border-bottom: 0.5px solid var(--rule);
    scrollbar-width: none;
  }
  .ios nav::-webkit-scrollbar {
    display: none;
  }
  .ios nav h2,
  .ios nav .version {
    display: none;
  }
  .ios nav button {
    flex: none;
    white-space: nowrap;
    padding: 8px 15px;
    border-radius: 18px;
    background: var(--hover);
  }
  .ios nav button.on {
    background: var(--accent);
    color: var(--accent-ink);
  }
  .ios section {
    flex: 1;
    padding: 6px 18px calc(env(safe-area-inset-bottom) + 30px);
    -webkit-overflow-scrolling: touch;
  }
  .ios .line {
    flex-wrap: wrap;
  }
  .ios .presets {
    flex-wrap: wrap;
  }
  .ios .eq {
    gap: 0;
    justify-content: space-between;
  }
  .ios .text,
  .ios select {
    height: 44px;
    font-size: 16px;
    width: 100%;
  }
  .ios .pill-btn {
    height: 42px;
  }
  .ios .grid2 {
    grid-template-columns: 1fr;
  }
  .ios .range input {
    min-width: 0;
  }
  .grid2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
    margin: 10px 0;
  }
  .grid2 .text {
    min-width: 0;
  }
  .bad {
    color: #d0675a;
  }
  .ok {
    color: var(--accent);
    font-size: 13px;
  }
  button:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
