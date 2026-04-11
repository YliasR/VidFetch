<script lang="ts">
  import Header from './Header.svelte';
  import Sidebar from './Sidebar.svelte';
  import MainPane from './MainPane.svelte';
</script>

<div class="shell">
  <Header />
  <div class="body">
    <Sidebar />
    <main class="main">
      <MainPane />
    </main>
  </div>
</div>

<style>
  .shell {
    display: flex;
    flex-direction: column;
    height: 100vh;
    width: 100vw;
    overflow: hidden;
  }

  .body {
    display: flex;
    flex: 1;
    min-height: 0;
  }

  .main {
    flex: 1;
    min-width: 0;
    overflow: auto;
    padding: 28px 32px;
  }
</style>
