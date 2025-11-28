<script>
  import { store } from '../lib/store.svelte';
  import { ICONS } from '../lib/constants';
  import { onMount } from 'svelte';
  import { slide } from 'svelte/transition';
  import Skeleton from '../components/Skeleton.svelte';
  import './ModulesTab.css';

  let searchQuery = $state('');
  let filterType = $state('all'); 
  let expandedMap = $state({}); // Track expanded modules by ID

  onMount(() => {
    store.loadModules();
  });

  let filteredModules = $derived(store.modules.filter(m => {
    const q = searchQuery.toLowerCase();
    const matchSearch = m.name.toLowerCase().includes(q) || m.id.toLowerCase().includes(q);
    const matchFilter = filterType === 'all' || m.mode === filterType;
    return matchSearch && matchFilter;
  }));

  function toggleExpand(id) {
    if (expandedMap[id]) {
      delete expandedMap[id];
    } else {
      expandedMap[id] = true;
    }
    // Re-assign to trigger reactivity in Svelte 5 rune
    expandedMap = { ...expandedMap };
  }

  function handleKeydown(e, id) {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      toggleExpand(id);
    }
  }
</script>

<div class="md3-card" style="padding: 16px;">
  <p style="margin: 0; font-size: 14px; color: var(--md-sys-color-on-surface-variant); line-height: 1.5;">
    {store.L.modules.desc}
  </p>
</div>

<div class="search-container">
  <svg class="search-icon" viewBox="0 0 24 24"><path d={ICONS.search} /></svg>
  <input 
    type="text" 
    class="search-input" 
    placeholder={store.L.modules.searchPlaceholder}
    bind:value={searchQuery}
  />
  <div class="filter-controls">
    <span style="font-size: 12px; color: var(--md-sys-color-on-surface-variant);">{store.L.modules.filterLabel}</span>
    <select class="filter-select" bind:value={filterType}>
      <option value="all">{store.L.modules.filterAll}</option>
      <option value="auto">{store.L.modules.modeAuto}</option>
      <option value="magic">{store.L.modules.modeMagic}</option>
    </select>
  </div>
</div>

{#if store.loading.modules}
  <div class="rules-list">
    {#each Array(5) as _}
      <div class="rule-card">
        <div class="rule-info">
          <div style="display:flex; flex-direction:column; gap: 6px; width: 100%;">
            <Skeleton width="60%" height="20px" />
            <Skeleton width="40%" height="14px" />
          </div>
        </div>
        <Skeleton width="120px" height="40px" borderRadius="4px" />
      </div>
    {/each}
  </div>
{:else if filteredModules.length === 0}
  <div style="text-align:center; padding: 40px; opacity: 0.6">
    {store.modules.length === 0 ? store.L.modules.empty : "No matching modules"}
  </div>
{:else}
  <div class="rules-list">
    {#each filteredModules as mod (mod.id)}
      <div 
        class="rule-card" 
        class:expanded={expandedMap[mod.id]} 
        onclick={() => toggleExpand(mod.id)}
        onkeydown={(e) => handleKeydown(e, mod.id)}
        role="button"
        tabindex="0"
      >
        <div class="rule-main">
          <div class="rule-info">
            <div style="display:flex; flex-direction:column;">
              <span class="module-name">{mod.name}</span>
              <span class="module-id">{mod.id} <span style="opacity:0.6; margin-left: 8px;">{mod.version}</span></span>
            </div>
          </div>
          <div 
            class="text-field" 
            style="margin-bottom:0; width: 140px; flex-shrink: 0;" 
            onclick={(e) => e.stopPropagation()}
            onkeydown={(e) => e.stopPropagation()}
            role="group"
            tabindex="-1"
          >
            <select bind:value={mod.mode}>
              <option value="auto">{store.L.modules.modeAuto}</option>
              <option value="magic">{store.L.modules.modeMagic}</option>
            </select>
          </div>
        </div>
        
        {#if expandedMap[mod.id]}
          <div class="rule-details" transition:slide={{ duration: 200 }}>
            <p class="module-desc">{mod.description || 'No description'}</p>
            <p class="module-meta">Author: {mod.author || 'Unknown'}</p>
          </div>
        {/if}
      </div>
    {/each}
  </div>
{/if}

<div class="bottom-actions">
  <button class="btn-tonal" onclick={() => store.loadModules()} disabled={store.loading.modules} title={store.L.modules.reload}>
    <svg viewBox="0 0 24 24" width="20" height="20"><path d={ICONS.refresh} fill="currentColor"/></svg>
  </button>
  <button class="btn-filled" onclick={() => store.saveModules()} disabled={store.saving.modules}>
    <svg viewBox="0 0 24 24" width="18" height="18"><path d={ICONS.save} fill="currentColor"/></svg>
    {store.saving.modules ? store.L.common.saving : store.L.modules.save}
  </button>
</div>