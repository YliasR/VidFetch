<script lang="ts">
</script>

<div class="splash">
  <div class="pulse"></div>
</div>

<style>
  .splash {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100vh;
    width: 100vw;
    background: var(--bg);
  }

  .pulse {
    width: 40px;
    height: 40px;
    border-radius: 10px;
    background: linear-gradient(135deg, var(--accent), var(--accent-strong));
    animation: pulse 1.2s ease-in-out infinite;
  }

  @keyframes pulse {
    0%,
    100% {
      transform: scale(0.9);
      opacity: 0.5;
    }
    50% {
      transform: scale(1.05);
      opacity: 1;
    }
  }
</style>
