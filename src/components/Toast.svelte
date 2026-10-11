<script lang="ts">
  import { fly } from "svelte/transition";
  import { player, ui } from "../lib/state.svelte";
</script>

{#if ui.toast}
  <div class="toast glass" class:lifted={player.status.track_id != null} transition:fly={{ y: 16, duration: 300 }} role="status">{ui.toast}</div>
{/if}

<style>
  .toast {
    position: fixed;
    left: 50%;
    bottom: 24px;
    translate: -50% 0;
    z-index: 95;
    max-width: min(560px, 80vw);
    padding: 10px 18px;
    border-radius: 999px;
    background: var(--glass-strong);
    font-size: 13px;
    transition: bottom 0.4s var(--spring);
  }
  .toast.lifted {
    bottom: 104px;
  }
</style>
