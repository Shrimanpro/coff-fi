<script lang="ts">
  import { api } from "../lib/ipc";
  import { cycleRepeat, player, queue, toggleShuffle } from "../lib/state.svelte";
  import Icon from "./Icon.svelte";

  let { big = false }: { big?: boolean } = $props();
  const s = $derived(big ? 1.35 : 1);
  const repeat = $derived(queue.repeat);
</script>

<div class="transport" class:big role="group" aria-label="Playback controls">
  <button class="icon-btn" class:on={queue.shuffle} title="Shuffle" onclick={toggleShuffle}><Icon name="shuffle" size={17 * s} /></button>
  <button class="icon-btn" title="Previous" onclick={() => api.cmd("prev")}><Icon name="prev" size={20 * s} /></button>
  <button class="icon-btn main" title={player.status.playing ? "Pause" : "Play"} onclick={() => api.cmd("toggle")}>
    <Icon name={player.status.playing ? "pause" : "play"} size={26 * s} />
  </button>
  <button class="icon-btn" title="Next" onclick={() => api.cmd("next")}><Icon name="next" size={20 * s} /></button>
  <button class="icon-btn" class:on={repeat !== "off"} title="Repeat: {repeat}" onclick={cycleRepeat}>
    <Icon name={repeat === "one" ? "repeat-one" : "repeat"} size={17 * s} />
  </button>
</div>

<style>
  .transport {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .transport .icon-btn {
    color: var(--text);
  }
  .transport .icon-btn:not(.main):not(.on) {
    color: var(--text-2);
  }
  .transport .on {
    color: var(--accent);
  }
  .main {
    width: 42px;
    height: 42px;
  }
  .big {
    gap: 14px;
  }
  .big .icon-btn {
    width: 46px;
    height: 46px;
  }
  .big .main {
    width: 60px;
    height: 60px;
  }
</style>
