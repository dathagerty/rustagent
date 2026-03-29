<script lang="ts">
  /**
   * LoadingSpinner component.
   * A simple CSS-based spinner animation.
   * Accepts optional size prop (default: 24px).
   */

  type Props = {
    size?: string;
  };

  let { size = '24px' }: Props = $props();
</script>

<div class="spinner" style="width: {size}; height: {size}">
  <div class="spinner-inner"></div>
</div>

<style>
  .spinner {
    display: inline-block;
  }

  .spinner-inner {
    width: 100%;
    height: 100%;
    border: 3px solid #4b5563;
    border-top-color: #3b82f6;
    border-radius: 50%;
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
