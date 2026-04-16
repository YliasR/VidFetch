<script lang="ts">
  import { currentView } from '$lib/stores/nav';
  import DownloadView from './DownloadView.svelte';
  import QueueView from './QueueView.svelte';
  import SettingsView from './SettingsView.svelte';
  import PlaceholderView from './PlaceholderView.svelte';
</script>

{#if $currentView === 'download'}
  <DownloadView />
{:else if $currentView === 'queue'}
  <QueueView />
{:else if $currentView === 'history'}
  <PlaceholderView title="History" description="Everything you've downloaded, with re-download and open-folder shortcuts." />
{:else if $currentView === 'presets'}
  <PlaceholderView title="Presets" description="Saved configurations: pick a preset and hit download — no fiddling." />
{:else if $currentView === 'settings'}
  <SettingsView />
{/if}
