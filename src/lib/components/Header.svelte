<script lang="ts">
  import ThemeSwitcher from './ThemeSwitcher.svelte';
</script>

<header class="header">
  <div class="brand">
    <div class="logo-glyph" aria-hidden="true">
      <svg viewBox="0 0 32 32" width="26" height="26">
        <defs>
          <linearGradient id="g" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" stop-color="var(--accent)" />
            <stop offset="1" stop-color="var(--accent-strong)" />
          </linearGradient>
        </defs>
        <rect x="2" y="2" width="28" height="28" rx="7" fill="url(#g)" />
        <path d="M10 12 L16 22 L22 12" stroke="var(--accent-fg)" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" fill="none" />
      </svg>
    </div>
    <h1>VidFetch</h1>
  </div>

  <div class="spacer"></div>

  <ThemeSwitcher />
</header>

<style>
  .header {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 14px 20px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    user-select: none;
    -webkit-app-region: drag;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .brand h1 {
    font-size: 16px;
    font-weight: 600;
    margin: 0;
    letter-spacing: 0.2px;
  }

  .logo-glyph {
    display: flex;
  }

  .spacer {
    flex: 1;
  }

  :global(.header > :not(.brand):not(.spacer)) {
    -webkit-app-region: no-drag;
  }
</style>
