import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({
      pages: 'build',
      assets: 'build',
      // SPA-style fallback so the rust server can serve client-side routes
      // via the `index.html`-fallback rule in `sppl::resolve`.
      fallback: 'index.html',
      precompress: false,
      strict: true
    })
  }
};

export default config;
