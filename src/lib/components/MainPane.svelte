<script lang="ts">
  import { currentView } from '$lib/stores/nav';
  import DownloadView from './DownloadView.svelte';
  import PlaceholderView from './PlaceholderView.svelte';
</script>

{#if $currentView === 'download'}
  <DownloadView />
{:else if $currentView === 'queue'}
  <PlaceholderView title="Queue" description="Active and queued downloads with pause / resume / cancel controls." />
{:else if $currentView === 'history'}
  <PlaceholderView title="History" description="Everything you've downloaded, with re-download and open-folder shortcuts." />
{:else if $currentView === 'presets'}
  <PlaceholderView title="Presets" description="Saved configurations: pick a preset and hit download — no fiddling." />
{:else if $currentView === 'settings'}
  <PlaceholderView title="Settings" description="yt-dlp + ffmpeg versions, update controls, output defaults, cookies, theme, and more." />
{/if}
