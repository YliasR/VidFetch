<script lang="ts">
  export let title: string;
  export let description: string;
</script>

<section class="view">
  <h2>{title}</h2>
  <p class="muted">{description}</p>
  <div class="card hint">
    <span class="badge">Coming soon</span>
    <p>This view is a placeholder — it gets wired up in a later phase.</p>
  </div>
</section>

<style>
  .view {
    max-width: 880px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  h2 {
    font-size: 24px;
    font-weight: 650;
    margin: 0;
    letter-spacing: -0.2px;
  }

  .hint {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .hint p {
    margin: 0;
    color: var(--fg-muted);
  }

  .badge {
    display: inline-block;
    width: fit-content;
    padding: 3px 10px;
    border-radius: 999px;
    background: var(--accent-muted);
    color: var(--accent);
    font-size: 11.5px;
    font-weight: 600;
    letter-spacing: 0.3px;
    text-transform: uppercase;
  }
</style>
