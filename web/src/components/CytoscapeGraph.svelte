<script lang="ts">
  /**
   * CytoscapeGraph wrapper component.
   * A reusable Svelte 5 wrapper around Cytoscape.js with dagre layout support.
   * Handles initialization, reactive updates, cleanup, and exposes methods for external control.
   */

  import { onMount } from 'svelte';
  import cytoscape from 'cytoscape';
  import type { ElementDefinition, Stylesheet } from 'cytoscape';
  import dagre from 'cytoscape-dagre';

  // Register the dagre layout extension at module level, once
  cytoscape.use(dagre);

  type Props = {
    elements: Array<ElementDefinition>;
    stylesheet: Array<Stylesheet>;
    layoutOptions?: Record<string, unknown>;
    onNodeClick?: (nodeId: string) => void;
    onBackgroundClick?: () => void;
  };

  let {
    elements,
    stylesheet,
    layoutOptions = { name: 'dagre', rankDir: 'TB', spacingFactor: 1.5 },
    onNodeClick,
    onBackgroundClick,
  }: Props = $props();

  let containerDiv: HTMLDivElement | undefined = $state();
  let cyInstance: cytoscape.Core | null = $state(null);

  onMount(() => {
    if (!containerDiv) return;

    // Initialize cytoscape instance
    cyInstance = cytoscape({
      container: containerDiv,
      elements: elements,
      style: stylesheet,
      layout: layoutOptions as cytoscape.LayoutOptions,
      pixelRatio: 1,
      wheelSensitivity: 0.2,
    });

    // Register event handlers
    cyInstance.on('tap', 'node', (event: cytoscape.EventObject) => {
      const nodeId = event.target.id();
      onNodeClick?.(nodeId);
    });

    cyInstance.on('tap', (event: cytoscape.EventObject) => {
      // Only trigger background click if target is the background (not a node/edge)
      if (event.target === cyInstance) {
        onBackgroundClick?.();
      }
    });

    // Return cleanup function
    return () => {
      if (cyInstance) {
        cyInstance.destroy();
        cyInstance = null;
      }
    };
  });

  // Reactive updates when elements change
  $effect(() => {
    if (!cyInstance) return;

    cyInstance.batch(() => {
      cyInstance!.elements().remove();
      cyInstance!.add(elements);
    });

    // Re-run layout after adding elements
    const layout = cyInstance.layout(layoutOptions as cytoscape.LayoutOptions);
    layout.run();
  });

  // Reactive updates when stylesheet changes
  $effect(() => {
    if (!cyInstance) return;
    cyInstance.style(stylesheet);
  });

  /**
   * Fit the entire graph to the viewport with padding.
   */
  export function fitView(): void {
    if (cyInstance) {
      cyInstance.fit(undefined, 20);
    }
  }

  /**
   * Get the underlying Cytoscape instance for advanced use cases.
   */
  export function getInstance(): cytoscape.Core | null {
    return cyInstance;
  }
</script>

<div class="cytoscape-container" bind:this={containerDiv}></div>

<style>
  .cytoscape-container {
    width: 100%;
    height: 100%;
    background-color: #f5f5f5;
    position: relative;
  }
</style>
