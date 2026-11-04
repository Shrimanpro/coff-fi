<script lang="ts">
  import Icon from "../../components/Icon.svelte";
  import { push } from "../nav.svelte";
  import PageHead from "../PageHead.svelte";

  const ROWS = [
    { which: "artists", label: "Artists", icon: "artist" },
    { which: "albums", label: "Albums", icon: "album" },
    { which: "songs", label: "Songs", icon: "music" },
    { which: "folders", label: "Folders", icon: "folder" },
  ] as const;
</script>

<PageHead title="Library" />

<ul>
  {#each ROWS as r (r.which)}
    <li>
      <button onclick={() => push({ type: "list", which: r.which })}>
        <span class="ic"><Icon name={r.icon} size={22} /></span>
        <span class="label">{r.label}</span>
        <Icon name="chev-right" size={17} />
      </button>
    </li>
  {/each}
</ul>

<style>
  ul {
    list-style: none;
    margin: 0;
    padding: 0 0 0 18px;
  }
  button {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 14px;
    height: 52px;
    padding-right: 18px;
    font-size: 19px;
    color: var(--text-3);
    border-bottom: 0.5px solid var(--rule);
    -webkit-tap-highlight-color: transparent;
  }
  button:active {
    background: var(--hover);
  }
  .ic {
    color: var(--accent-2);
    display: grid;
  }
  :global([data-theme="light"]) .ic {
    color: var(--accent);
  }
  .label {
    flex: 1;
    text-align: left;
    color: var(--text);
  }
</style>
