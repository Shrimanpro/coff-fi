<script lang="ts">
  // Pick where music plays: this device or another Coff-fi on the network.
  import { haptic } from "../lib/haptics";
  import { deviceKind, remote } from "../lib/remote.svelte";
  import { controlDevice, ui } from "../lib/state.svelte";
  import type { Device } from "../lib/types";
  import Icon from "./Icon.svelte";

  const ios = $derived(ui.platform === "ios");
  const here = $derived(ui.platform === "ios" ? "This iPhone" : ui.platform === "macos" ? "This Mac" : "This computer");
  let address = $state("");
  let manual = $state(false);

  $effect(() => {
    if (remote.picker) remote.error = null;
  });

  const icon = (platform: string) => (platform === "ios" ? "phone" : "laptop");

  function pick(d: Device | null) {
    haptic("light");
    if (remote.connecting !== null) return;
    if ((d?.id ?? null) === (remote.target?.id ?? null)) {
      remote.picker = false;
      return;
    }
    controlDevice(d);
  }

  function connectAddress(e: Event) {
    e.preventDefault();
    if (!address.trim()) return;
    controlDevice(null, address.trim());
  }

  // The target may not be in the Bonjour list (connected by address), but should still show.
  const list = $derived(
    remote.target && !remote.devices.some((d) => d.id === remote.target?.id) ? [remote.target, ...remote.devices] : remote.devices,
  );
</script>

{#if remote.picker}
  <!-- Not ".scrim": mobile.css lifts every .scrim to z-index 150 for the settings sheet. -->
  <div class="pick-scrim" role="presentation" onclick={() => remote.connecting === null && (remote.picker = false)}></div>
  <div class="picker" class:ios role="dialog" aria-label="Choose a device">
    <div class="head">
      <h3>Play on</h3>
      <button class="close icon-btn" aria-label="Close" onclick={() => (remote.picker = false)}><Icon name="close" size={16} /></button>
    </div>
    <button class="dev" class:on={!remote.target} onclick={() => pick(null)}>
      <Icon name={ios ? "phone" : "laptop"} size={22} />
      <span class="txt">
        <span class="name ellipsis">{here}</span>
        <span class="sub ellipsis">{remote.name}</span>
      </span>
      {#if !remote.target}<Icon name="check" size={18} />{/if}
    </button>
    {#each list as d (d.id)}
      <button class="dev" class:on={remote.target?.id === d.id} disabled={remote.connecting !== null} onclick={() => pick(d)}>
        <Icon name={icon(d.platform)} size={22} />
        <span class="txt">
          <span class="name ellipsis">{d.name}</span>
          <span class="sub ellipsis">{remote.connecting === d.id ? "Connecting…" : deviceKind(d.platform) + " · Coff-fi"}</span>
        </span>
        {#if remote.target?.id === d.id}<Icon name="check" size={18} />{/if}
      </button>
    {/each}
    {#if remote.error}
      <p class="err">{remote.error}</p>
    {/if}
    {#if !list.length}
      <p class="hint">Looking for other devices… Open Coff-fi on your other device, on the same Wi-Fi.</p>
    {/if}
    {#if manual}
      <form class="addr" onsubmit={connectAddress}>
        <input placeholder="192.168.1.20" bind:value={address} autocapitalize="off" autocomplete="off" spellcheck="false" />
        <button class="go" type="submit" disabled={remote.connecting !== null}>{remote.connecting ? "…" : "Connect"}</button>
      </form>
    {:else}
      <button class="link" onclick={() => (manual = true)}>Connect by address…</button>
    {/if}
  </div>
{/if}

<style>
  /* Above Now Playing (100), the settings sheet (151) and action sheets (201). */
  .pick-scrim {
    position: fixed;
    inset: 0;
    z-index: 300;
    background: rgba(0, 0, 0, 0.3);
  }
  .picker {
    position: fixed;
    z-index: 301;
    background: var(--bg-2);
    border: 1px solid var(--glass-border);
    box-shadow: var(--shadow);
    right: 24px;
    bottom: 100px;
    width: 320px;
    max-height: 70vh;
    overflow-y: auto;
    padding: 14px 10px 12px;
    border-radius: 20px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    animation: up 0.3s var(--ease);
  }
  .picker.ios {
    left: 10px;
    right: 10px;
    width: auto;
    bottom: calc(10px + env(safe-area-inset-bottom));
    border-radius: 28px;
    padding: 18px 12px 14px;
  }
  @keyframes up {
    from {
      transform: translateY(16px);
      opacity: 0;
    }
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 8px 8px;
  }
  h3 {
    margin: 0;
    font-size: 17px;
  }
  .dev {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px;
    border-radius: 12px;
    text-align: left;
    color: var(--text);
    -webkit-tap-highlight-color: transparent;
  }
  .dev:hover:not(:disabled) {
    background: color-mix(in srgb, var(--text) 7%, transparent);
  }
  .dev.on {
    color: var(--accent);
  }
  .dev:disabled {
    opacity: 0.6;
  }
  .txt {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .name {
    font-weight: 650;
    font-size: 15px;
  }
  .sub {
    font-size: 12.5px;
    color: var(--text-2);
  }
  .err {
    margin: 6px 10px;
    font-size: 13px;
    line-height: 1.35;
    color: #d0574f;
  }
  .hint {
    margin: 6px 10px;
    font-size: 13px;
    color: var(--text-2);
  }
  .link {
    align-self: flex-start;
    margin: 6px 10px 0;
    font-size: 13px;
    color: var(--accent);
  }
  .addr {
    display: flex;
    gap: 8px;
    margin: 6px 8px 0;
  }
  .addr input {
    flex: 1;
    min-width: 0;
    padding: 8px 10px;
    border-radius: 10px;
    border: 1px solid color-mix(in srgb, var(--text) 15%, transparent);
    background: color-mix(in srgb, var(--text) 5%, transparent);
    color: var(--text);
    font: inherit;
    font-size: 16px;
  }
  .go {
    padding: 0 14px;
    border-radius: 10px;
    background: var(--accent);
    color: #fff;
    font-weight: 650;
  }
</style>
