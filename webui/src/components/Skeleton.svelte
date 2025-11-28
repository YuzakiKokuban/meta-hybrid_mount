<script>
  let { width = '100%', height = '20px', style = '', borderRadius = '4px' } = $props();
</script>

<div class="skeleton" style="width: {width}; height: {height}; border-radius: {borderRadius}; {style}"></div>

<style>
  .skeleton {
    background-color: var(--md-sys-color-surface-variant);
    opacity: 0.3;
    animation: pulse 1.5s infinite ease-in-out;
    flex-shrink: 0;
  }

  @keyframes pulse {
    0% { opacity: 0.1; }
    50% { opacity: 0.3; }
    100% { opacity: 0.1; }
  }
</style>