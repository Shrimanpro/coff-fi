<script lang="ts">
  // "Playing on <device>" while controlling another device, "<device> is controlling this
  // device" while being controlled. Tapping it opens the device picker.
  import { remote } from "../lib/remote.svelte";
  import Icon from "./Icon.svelte";

  let { tone = "default" }: { tone?: "default" | "light" } = $props();
  const text = $derived(
    remote.target
      ? `Playing on ${remote.target.name}`
      : remote.controllers.length
        ? `${remote.controllers.join(", ")} ${remote.controllers.length > 1 ? "are" : "is"} controlling this device`
        : "",
  );
</script>

{#if text}
  <button
    class="badge"
    class:light={tone === "light"}
    class:remote={!!remote.target}
    onclick={(e) => (e.stopPropagation(), (remote.picker = true))}
    title={text}
  >
    <Icon name="devices" size={14} />
    <span class="ellipsis">{text}</span>
  </button>
{/if}

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    max-width: 100%;
    min-width: 0;
    padding: 3px 9px;
    border-radius: 999px;
    font-size: 11.5px;
    font-weight: 650;
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 16%, transparent);
    -webkit-tap-highlight-color: transparent;
  }
  .badge :global(svg) {
    flex: none;
  }
  .light {
    color: #fff;
    background: rgba(255, 255, 255, 0.16);
  }
  .light.remote {
    background: color-mix(in srgb, var(--accent) 55%, transparent);
  }
</style>
