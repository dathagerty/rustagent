<script lang="ts">
  /**
   * Placeholder component for views under development.
   * Renders a simple message indicating the view is coming soon.
   */

  interface Props {
    name: string;
  }

  let { name }: Props = $props();
</script>

<div class="placeholder">
  <h2>{name}</h2>
  <p>Coming soon...</p>
</div>

<style>
  .placeholder {
    padding: 2rem;
    text-align: center;
    color: #999;
  }

  h2 {
    margin: 0 0 1rem 0;
    color: #e0e0e0;
  }

  p {
    margin: 0;
  }
</style>
