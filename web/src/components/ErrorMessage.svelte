<script lang="ts">
  /**
   * ErrorMessage component.
   * Displays a styled error box with the provided message.
   */

  type Props = {
    message: string;
  };

  let { message }: Props = $props();
</script>

<div class="error-box">
  <div class="error-icon">!</div>
  <div class="error-text">{message}</div>
</div>

<style>
  .error-box {
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 1rem;
    background-color: #7f1d1d;
    border: 1px solid #dc2626;
    border-radius: 0.5rem;
    color: #fecaca;
  }

  .error-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background-color: #dc2626;
    color: white;
    font-weight: bold;
    flex-shrink: 0;
  }

  .error-text {
    flex: 1;
    font-size: 0.875rem;
  }
</style>
