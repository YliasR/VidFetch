<script lang="ts">
  import { onMount } from 'svelte';
  import Shell from '$lib/components/Shell.svelte';
  import FirstRunWizard from '$lib/components/FirstRunWizard.svelte';
  import BootSplash from '$lib/components/BootSplash.svelte';
  import { initTheme } from '$lib/stores/theme';
  import { ytdlpStore, bootCheck } from '$lib/stores/ytdlp';

  onMount(async () => {
    await initTheme();
    await bootCheck();
  });

  $: boot = $ytdlpStore.boot;
</script>

{#if boot === 'checking'}
  <BootSplash />
{:else if boot === 'ready'}
  <Shell />
{:else}
  <FirstRunWizard />
{/if}
