<script>
  import { store } from '../lib/store.svelte';
  import { ICONS } from '../lib/constants';
  import { onMount, tick } from 'svelte';
  import Skeleton from '../components/Skeleton.svelte';
  import './LogsTab.css';

  let searchLogQuery = $state('');
  let filterLevel = $state('all'); // all, info, warn, error
  let logContainer; // Reference to the DOM element

  // Derived state: Filter logs based on search query and level
  let filteredLogs = $derived(store.logs.filter(line => {
    const text = line.text.toLowerCase();
    const matchesSearch = text.includes(searchLogQuery.toLowerCase());
    
    let matchesLevel = true;
    if (filterLevel !== 'all') {
      matchesLevel = line.type === filterLevel;
    }
    
    return matchesSearch && matchesLevel;
  }));

  // Auto-scroll function
  async function scrollToBottom() {
    if (logContainer) {
      await tick(); // Wait for DOM update
      logContainer.scrollTop = logContainer.scrollHeight;
    }
  }

  // Load logs and scroll
  async function refreshLogs() {
    await store.loadLogs();
    scrollToBottom();
  }
  
  // Actions
  async function copyLogs() {
    if (filteredLogs.length === 0) return;
    const text = filteredLogs.map(l => l.text).join('\n');
    try {
      await navigator.clipboard.writeText(text);
      store.showToast(store.L.logs.copySuccess, 'success');
    } catch (e) {
      store.showToast(store.L.logs.copyFail, 'error');
    }
  }

  onMount(() => {
    refreshLogs();
  });
</script>

<div class="logs-controls">
  <svg viewBox="0 0 24 24" width="20" height="20" style="fill: var(--md-sys-color-on-surface-variant)">
    <path d={ICONS.search} />
  </svg>
  <input 
    type="text" 
    class="log-search-input" 
    placeholder={store.L.logs.searchPlaceholder}
    bind:value={searchLogQuery}
  />
  
  <div style="height: 16px; width: 1px; background: var(--md-sys-color-outline-variant); margin: 0 8px;"></div>

  <span style="font-size: 12px; color: var(--md-sys-color-on-surface-variant); white-space: nowrap;">
    {store.L.logs.filterLabel}
  </span>
  <select class="log-filter-select" bind:value={filterLevel}>
    <option value="all">{store.L.logs.levels.all}</option>
    <option value="info">{store.L.logs.levels.info}</option>
    <option value="warn">{store.L.logs.levels.warn}</option>
    <option value="error">{store.L.logs.levels.error}</option>
  </select>
</div>

<div class="log-container" bind:this={logContainer}>
  {#if store.loading.logs}
    <div style="display:flex; flex-direction:column; gap:8px;">
      {#each Array(10) as _, i}
        <Skeleton width="{60 + (i % 3) * 20}%" height="14px" />
      {/each}
    </div>
  {:else if filteredLogs.length === 0}
    <div style="padding: 20px; text-align: center;">
      {store.logs.length === 0 ? store.L.logs.empty : "No matching logs"}
    </div>
  {:else}
    {#each filteredLogs as line}
      <span class="log-entry">
        <span class="log-{line.type}">{line.text}</span>
      </span>
    {/each}
  {/if}
</div>

<div class="bottom-actions">
  <button class="btn-tonal" onclick={copyLogs} disabled={filteredLogs.length === 0} title={store.L.logs.copy}>
    <svg viewBox="0 0 24 24" width="20" height="20"><path d={ICONS.copy} fill="currentColor"/></svg>
  </button>
  <div style="flex:1"></div>
  <button class="btn-filled" onclick={refreshLogs} disabled={store.loading.logs}>
    <svg viewBox="0 0 24 24" width="18" height="18"><path d={ICONS.refresh} fill="currentColor"/></svg>
    {store.L.logs.refresh}
  </button>
</div>