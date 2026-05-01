<script lang="ts">
  import {
    ping,
    getStatus,
    getAssets,
    formatUptime,
    formatBytes,
    type Status,
    type Assets,
  } from '$lib/api';

  let api = $state<string | null>(null);
  let err = $state<string | null>(null);

  let status = $state<Status | null>(null);
  let statusErr = $state<string | null>(null);

  let assets = $state<Assets | null>(null);

  let bars = $derived(status?.buckets ?? Array(30).fill(0));
  let peak = $derived(Math.max(1, ...bars));

  async function pingClick() {
    err = null;
    try {
      api = await ping();
    } catch (e) {
      err = String(e);
    }
  }

  async function pollStatus() {
    try {
      status = await getStatus();
      statusErr = null;
    } catch (e) {
      statusErr = String(e);
    }
  }

  $effect(() => {
    pollStatus();
    getAssets()
      .then((a) => (assets = a))
      .catch(() => {});
    const id = setInterval(pollStatus, 2000);
    return () => clearInterval(id);
  });
</script>

<h1>sppl</h1>

<p>
  Embed <mark>static Svelte apps</mark> directly into your Rust binary. This page is
  a Svelte 5 component, prerendered to HTML and baked into the server at compile
  time.
</p>

<h2>requests · last 30s</h2>

<p>
  One bar per second, oldest on the left, current second on the right:
</p>

<div class="graph" aria-label="requests per second, last 30 seconds">
  {#each bars as n, i (i)}
    <div class="bar-wrap" title="-{bars.length - 1 - i}s · {n} req">
      <div class="bar" style:height="{(n / peak) * 100}%"></div>
    </div>
  {/each}
</div>
<div class="axis">
  <span>−30s</span>
  <span>now</span>
</div>

<h2>server</h2>

<p>
  Ping the rust handler at <mark>/api/hello</mark>, served by the same axum
  router as this page:
</p>

<p>
  <button onclick={pingClick}>ping rust</button>
</p>

{#if api}
  <pre>{api}</pre>
{/if}
{#if err}
  <pre class="err">{err}</pre>
{/if}

<h2>status</h2>

<p>
  Polling <mark>/api/status</mark> every 2s:
</p>

<dl>
  <dt>pid</dt>
  <dd>{status?.pid ?? '—'}</dd>
  <dt>uptime</dt>
  <dd>{status ? formatUptime(status.uptime_secs) : '—'}</dd>
  <dt>epoch</dt>
  <dd>{status?.epoch_secs ?? '—'}</dd>
  <dt>requests</dt>
  <dd>{status?.requests ?? '—'}</dd>
</dl>

{#if statusErr}
  <pre class="err">{statusErr}</pre>
{/if}

<h2>bundle</h2>

<p>
  What's actually baked into the binary, from <mark>/api/assets</mark>:
</p>

<dl>
  <dt>files</dt>
  <dd>{assets?.count ?? '—'}</dd>
  <dt>in binary</dt>
  <dd>{assets ? formatBytes(assets.bytes_in_binary) : '—'}</dd>
  <dt>raw size</dt>
  <dd>{assets ? formatBytes(assets.uncompressed_bytes) : '—'}</dd>
</dl>

<style>
  h1 {
    font-weight: 700;
    font-size: 3rem;
    line-height: 1.1;
    letter-spacing: -0.02em;
    margin: 4rem 0 1.5rem;
  }
  h2 {
    font-weight: 600;
    font-size: 1rem;
    margin-top: 2rem;
  }
  p + p {
    margin-top: 1.75rem;
  }
  pre {
    margin-top: 1rem;
    padding: 0.6rem 0.8rem;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--fg);
    overflow-x: auto;
  }
  pre.err {
    color: #a00;
  }
  dl {
    margin-top: 1rem;
    display: grid;
    grid-template-columns: 6rem 1fr;
    row-gap: 0.25rem;
    column-gap: 1rem;
  }
  dt {
    color: var(--muted);
  }
  dd {
    margin: 0;
  }

  .graph {
    margin-top: 1rem;
    display: grid;
    grid-template-columns: repeat(30, 1fr);
    gap: 2px;
    height: 8rem;
    border-bottom: 1px solid var(--border);
    padding-bottom: 0.25rem;
  }

  .bar-wrap {
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    align-items: stretch;
    min-height: 0;
  }

  .bar {
    background: var(--yellow);
    min-height: 1px;
    transition: height 250ms ease;
  }

  .axis {
    display: flex;
    justify-content: space-between;
    font-size: 0.75rem;
    color: var(--muted);
    margin-top: 0.25rem;
  }
</style>
