<script lang="ts">
  import { currentView } from '$lib/stores/nav';
  import DownloadView from './DownloadView.svelte';
  import QueueView from './QueueView.svelte';
  import SettingsView from './SettingsView.svelte';
  import HistoryView from './HistoryView.svelte';
  import PlaceholderView from './PlaceholderView.svelte';
</script>

{#if $currentView === 'download'}
  <DownloadView />
{:else if $currentView === 'queue'}
  <QueueView />
{:else if $currentView === 'history'}
  <HistoryView />
{:else if $currentView === 'presets'}
  <PlaceholderView title="Presets" description="Saved configurations: pick a preset and hit download — no fiddling." />
{:else if $currentView === 'settings'}
  <SettingsView />
{/if}
