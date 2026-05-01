<script lang="ts">
  let { children } = $props();
</script>

<div class="shell">
  <header>
    <a class="brand" href="/">
      <img src="/favicon.svg" alt="" width="24" height="24" />
      <span>sppl</span>
    </a>
    <nav class="nav-links">
      <a href="/" class="active">demo</a>
      <a href="https://github.com/neutral-engineering/sppl">github</a>
    </nav>
  </header>

  <main>
    {@render children()}
  </main>

  <footer>
    <span>© 2026 neutral engineering</span>
    <span>hi@neutral.engineering</span>
  </footer>
</div>

<style>
  :global(:root) {
    --fg: #111;
    --bg: #f5f2eb;
    --border: #d4d0c8;
    --muted: #666;
    --yellow: #fcc21b;
  }

  @media (prefers-color-scheme: dark) {
    :global(:root) {
      --fg: #e5e5e5;
      --bg: #111;
      --border: #333;
      --muted: #999;
      --yellow: #fcc21b;
    }
  }

  :global(html, body) {
    margin: 0;
    padding: 0;
  }

  :global(body) {
    font-family: 'IBM Plex Mono', monospace;
    color: var(--fg);
    background: var(--bg);
    font-size: 1rem;
    line-height: 1.7;
    -webkit-font-smoothing: antialiased;
  }

  :global(mark) {
    background: var(--yellow);
    color: #111;
    padding: 0.05rem 0.2rem;
  }

  :global(a) {
    color: inherit;
    text-decoration-color: var(--yellow);
  }

  :global(button) {
    font-family: inherit;
    font-size: 0.875rem;
    padding: 0.5rem 1.5rem;
    border: 1px solid var(--fg);
    background: var(--fg);
    color: var(--bg);
    cursor: pointer;
  }
  :global(button:hover) {
    background: #333;
    color: #e5e5e5;
  }

  :global(input, textarea) {
    font-family: inherit;
    font-size: 0.875rem;
    padding: 0.5rem;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--fg);
  }

  .shell {
    min-height: 100dvh;
    display: flex;
    flex-direction: column;
  }

  header {
    position: sticky;
    top: 0;
    background: var(--bg);
    border-bottom: 1px solid var(--border);
    padding: 1rem 1.5rem;
    display: flex;
    align-items: center;
    justify-content: space-between;
    z-index: 1;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-weight: 700;
    text-decoration: none;
  }

  .nav-links {
    display: flex;
    gap: 1.5rem;
    font-size: 0.875rem;
  }

  .nav-links a {
    text-decoration: none;
    padding: 0.15rem 0.4rem;
  }

  .nav-links a.active {
    background: var(--yellow);
    color: #111;
  }

  main {
    flex: 1;
    width: 100%;
    max-width: 640px;
    margin: 0 auto;
    padding: 2rem 1.5rem;
  }

  footer {
    border-top: 1px solid var(--border);
    padding: 1rem 1.5rem;
    display: flex;
    justify-content: space-between;
    font-weight: 700;
    font-size: 0.875rem;
    color: var(--muted);
  }

  @media (max-width: 600px) {
    header {
      padding: 0.75rem 1rem;
    }
    .brand span {
      font-size: 0.8rem;
    }
    .brand img {
      width: 2rem;
      height: 2rem;
    }
    .nav-links {
      gap: 0.75rem;
      font-size: 0.75rem;
    }
  }
</style>
