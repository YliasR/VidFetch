<script lang="ts">
  import Shell from '$lib/components/Shell.svelte';
  import { initTheme } from '$lib/stores/theme';
  import { onMount } from 'svelte';

  onMount(() => {
    initTheme();
  });
</script>

<Shell />
