<script lang="ts">
  /**
   * TreeNodeRow component.
   * Renders a single row in the hierarchical task tree.
   * Shows title, status/priority badges, agent assignment, dependencies, and expand/collapse toggle.
   */

  import type { TreeNode } from '../lib/tree';
  import StatusBadge from './StatusBadge.svelte';
  import PriorityBadge from './PriorityBadge.svelte';
  import TreeNodeRow from './TreeNodeRow.svelte';

  type Props = {
    treeNode: TreeNode;
    depth: number;
    isSelected?: boolean;
    onSelect?: (nodeId: string) => void;
    onToggleExpanded?: (nodeId: string) => void;
  };

  let { treeNode, depth, isSelected = false, onSelect, onToggleExpanded }: Props = $props();

  const node = $derived(treeNode.node);
  const hasChildren = $derived(treeNode.children.length > 0);
  const hasDependencies = $derived(treeNode.dependencies.length > 0);

  function handleClick(): void {
    onSelect?.(node.id);
  }

  function handleKeyDown(e: KeyboardEvent): void {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      handleClick();
    }
  }

  function handleToggle(e: Event): void {
    e.stopPropagation();
    onToggleExpanded?.(node.id);
  }

  const indentStyle = $derived(`padding-left: ${depth * 24}px`);
  const isReady = $derived(node.status === 'ready');
  const rowClass = $derived(`tree-row ${isSelected ? 'selected' : ''} ${isReady ? 'ready' : ''}`);
</script>

<div class={rowClass} style={indentStyle} role="button" tabindex="0" onclick={handleClick} onkeydown={handleKeyDown}>
  <div class="row-content">
    {#if hasChildren}
      <button class="expand-toggle" onclick={handleToggle} aria-label="Toggle children">
        <span class="toggle-arrow" class:expanded={treeNode.expanded}>▶</span>
      </button>
    {:else}
      <span class="expand-placeholder"></span>
    {/if}

    <div class="node-info">
      <span class="node-title">{node.title}</span>

      <div class="badges">
        <StatusBadge status={node.status} />
        <PriorityBadge priority={node.priority} />
      </div>

      {#if node.assigned_to}
        <span class="agent-label">{node.assigned_to}</span>
      {/if}

      {#if hasDependencies}
        <span class="dependency-indicator">
          blocked by: {treeNode.dependencies.map((d) => d.id).join(', ')}
        </span>
      {/if}
    </div>
  </div>
</div>

{#if treeNode.expanded && hasChildren}
  {#each treeNode.children as child (child.node.id)}
    <TreeNodeRow
      treeNode={child}
      depth={depth + 1}
      isSelected={isSelected && child.node.id === node.id}
      {onSelect}
      {onToggleExpanded}
    />
  {/each}
{/if}

<style>
  .tree-row {
    display: flex;
    flex-direction: column;
    cursor: pointer;
    user-select: none;
    border-left: 2px solid transparent;
    transition: background-color 0.15s ease, border-color 0.15s ease;
    font-family: 'Monaco', 'Menlo', 'Consolas', monospace;
    font-size: 0.875rem;
  }

  .tree-row:nth-child(odd) {
    background-color: #f9f9f9;
  }

  .tree-row:nth-child(even) {
    background-color: #ffffff;
  }

  .tree-row:hover {
    background-color: #f0f0f0;
  }

  .tree-row.selected {
    background-color: #e0e7ff;
    border-left-color: #3b82f6;
  }

  .tree-row.ready {
    background-color: #ecfdf5;
  }

  .row-content {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 0.75rem;
  }

  .expand-toggle {
    background: none;
    border: none;
    cursor: pointer;
    padding: 0;
    width: 1.5rem;
    height: 1.5rem;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #6b7280;
    transition: color 0.15s ease;
  }

  .expand-toggle:hover {
    color: #374151;
  }

  .toggle-arrow {
    display: inline-block;
    transition: transform 0.15s ease;
    transform: rotate(0deg);
    font-size: 0.75rem;
  }

  .toggle-arrow.expanded {
    transform: rotate(90deg);
  }

  .expand-placeholder {
    width: 1.5rem;
  }

  .node-info {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    flex: 1;
    min-width: 0;
  }

  .node-title {
    font-weight: 500;
    color: #111827;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .badges {
    display: flex;
    gap: 0.375rem;
    flex-shrink: 0;
  }

  .agent-label {
    background-color: #dbeafe;
    color: #1e40af;
    padding: 0.125rem 0.5rem;
    border-radius: 0.25rem;
    font-size: 0.75rem;
    font-weight: 500;
    flex-shrink: 0;
  }

  .dependency-indicator {
    background-color: #fee2e2;
    color: #991b1b;
    padding: 0.125rem 0.5rem;
    border-radius: 0.25rem;
    font-size: 0.75rem;
    font-weight: 500;
    flex-shrink: 0;
  }
</style>
