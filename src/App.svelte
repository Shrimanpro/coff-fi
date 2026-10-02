<script lang="ts">
  import Logo from "./components/Logo.svelte";
</script>

<main>
  <Logo size={120} />
  <h1>Coff-fi</h1>
</main>

<style>
  main {
    height: 100%;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 12px;
  }
</style>
