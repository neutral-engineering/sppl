<script lang="ts">
  let count = $state(0);
  let api = $state<string | null>(null);
  let err = $state<string | null>(null);

  async function ping() {
    try {
      const res = await fetch('/api/hello');
      api = await res.text();
    } catch (e) {
      err = String(e);
    }
  }
</script>

<main>
  <h1>sppl ✦ static svelte in rust</h1>
  <p>This page is a Svelte 5 component, prerendered to HTML and embedded into
    the Rust binary at compile time.</p>

  <button onclick={() => count++}>clicked {count} times</button>

  <hr />

  <p>Fetch from the rust server's <code>/api/hello</code>:</p>
  <button onclick={ping}>ping rust</button>
  {#if api}
    <pre>{api}</pre>
  {/if}
  {#if err}
    <pre style="color: tomato">{err}</pre>
  {/if}
</main>

<style>
  main {
    max-width: 36rem;
    margin: 4rem auto;
    padding: 0 1rem;
    font: 1rem/1.5 system-ui, sans-serif;
  }
  h1 { font-weight: 600; }
  button {
    font: inherit;
    padding: 0.4rem 0.8rem;
    border-radius: 0.4rem;
    border: 1px solid #888;
    background: #f7f7f7;
    cursor: pointer;
  }
  pre {
    background: #f0f0f0;
    padding: 0.6rem 0.8rem;
    border-radius: 0.4rem;
  }
</style>
