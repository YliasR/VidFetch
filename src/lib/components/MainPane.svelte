<script lang="ts">
  import { currentView } from '$lib/stores/nav';
  import DownloadView from './DownloadView.svelte';
  import QueueView from './QueueView.svelte';
  import EditView from './EditView.svelte';
  import SettingsView from './SettingsView.svelte';
  import HistoryView from './HistoryView.svelte';
  import PresetsView from './PresetsView.svelte';
</script>

{#if $currentView === 'download'}
  <DownloadView />
{:else if $currentView === 'queue'}
  <QueueView />
{:else if $currentView === 'edit'}
  <EditView />
{:else if $currentView === 'history'}
  <HistoryView />
{:else if $currentView === 'presets'}
  <PresetsView />
{:else if $currentView === 'settings'}
  <SettingsView />
{/if}
