<script lang="ts">
  import { store } from '../lib/store.svelte';
  import { ICONS } from '../lib/constants';
  import { onMount } from 'svelte';
  import { slide } from 'svelte/transition';
  import Skeleton from '../components/Skeleton.svelte';
  import BottomActions from '../components/BottomActions.svelte';
  import './ModulesTab.css';

  let searchQuery = $state('');
  let filterType = $state('all');
  let expandedId = $state<string | null>(null);
  
  let initialModulesStr = $state('');

  onMount(() => {
    load();
  });

  function load() {
    store.loadModules().then(() => {
        initialModulesStr = JSON.stringify(store.modules.map(m => ({ id: m.id, mode: m.mode })));
    });
  }

  let isDirty = $derived.by(() => {
    if (!initialModulesStr) return false;
    const current = JSON.stringify(store.modules.map(m => ({ id: m.id, mode: m.mode })));
    return current !== initialModulesStr;
  });

  function save() {
    store.saveModules().then(() => {
        initialModulesStr = JSON.stringify(store.modules.map(m => ({ id: m.id, mode: m.mode })));
    });
  }

  let filteredModules = $derived(store.modules.filter(m => {
    const q = searchQuery.toLowerCase();
    const matchSearch = m.name.toLowerCase().includes(q) || m.id.toLowerCase().includes(q);
    const matchFilter = filterType === 'all' || m.mode === filterType;
    return matchSearch && matchFilter;
  }));

  function toggleExpand(id: string) {
    expandedId = expandedId === id ? null : id;
  }

  function handleKeydown(e: KeyboardEvent, id: string) {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      toggleExpand(id);
    }
  }

  function getModeLabel(mode: string) {
      if (mode === 'magic') return store.L.modules.modeMagic;
      if (mode === 'hymofs') return "HymoFS";
      return store.L.modules.modeAuto;
  }
</script>

<div class="md3-card desc-card">
  <p class="desc-text">
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
    <span class="filter-label-text">{store.L.modules.filterLabel}</span>
    <select class="filter-select" bind:value={filterType}>
      <option value="all">{store.L.modules.filterAll}</option>
      <option value="auto">{store.L.modules.modeAuto}</option>
      <option value="magic">{store.L.modules.modeMagic}</option>
      {#if store.storage?.hymofs_available}
        <option value="hymofs">HymoFS</option>
      {/if}
    </select>
  </div>
</div>

{#if store.loading.modules}
  <div class="rules-list">
    {#each Array(5) as _}
      <div class="rule-card">
        <div class="rule-info">
          <div class="skeleton-group">
            <Skeleton width="60%" height="20px" />
            <Skeleton width="40%" height="14px" />
          </div>
        </div>
        <Skeleton width="120px" height="40px" borderRadius="4px" />
      </div>
    {/each}
  </div>
{:else if filteredModules.length === 0}
  <div class="empty-state">
    {store.modules.length === 0 ? store.L.modules.empty : "No matching modules"}
  </div>
{:else}
  <div class="rules-list">
    {#each filteredModules as mod (mod.id)}
      <div 
        class="rule-card" 
        class:expanded={expandedId === mod.id} 
        onclick={() => toggleExpand(mod.id)}
        onkeydown={(e) => handleKeydown(e, mod.id)}
        role="button"
        tabindex="0"
      >
        <div class="rule-main">
          <div class="rule-info">
            <div class="info-col">
              <span class="module-name">{mod.name}</span>
              <span class="module-id">{mod.id} <span class="version-tag">{mod.version}</span></span>
            </div>
          </div>
          
          <div class="mode-badge {mod.mode === 'magic' ? 'badge-magic' : mod.mode === 'hymofs' ? 'badge-hymofs' : 'badge-auto'}"
               style:background-color={mod.mode === 'hymofs' ? 'var(--md-sys-color-primary-container)' : ''}
               style:color={mod.mode === 'hymofs' ? 'var(--md-sys-color-on-primary-container)' : ''}>
            {getModeLabel(mod.mode)}
          </div>
        </div>
        
        {#if expandedId === mod.id}
          <div class="rule-details" transition:slide={{ duration: 200 }}>
            <p class="module-desc">{mod.description || 'No description'}</p>
            <p class="module-meta">Author: {mod.author || 'Unknown'}</p>
            
            <div class="config-section">
              <div class="config-row">
                <span class="config-label">{store.L.config.title}:</span>
                <div class="text-field compact-select">
                  <select 
                    bind:value={mod.mode}
                    onclick={(e) => e.stopPropagation()}
                    onkeydown={(e) => e.stopPropagation()}
                  >
                    <option value="auto">{store.L.modules.modeAuto}</option>
                    <option value="magic">{store.L.modules.modeMagic}</option>
                    {#if store.storage?.hymofs_available}
                      <option value="hymofs">HymoFS</option>
                    {/if}
                  </select>
                </div>
              </div>
            </div>

          </div>
        {/if}
      </div>
    {/each}
  </div>
{/if}

<BottomActions>
  <button class="btn-tonal" onclick={load} disabled={store.loading.modules} title={store.L.modules.reload}>
    <svg viewBox="0 0 24 24" width="20" height="20"><path d={ICONS.refresh} fill="currentColor"/></svg>
  </button>
  <div class="spacer"></div>
  <button class="btn-filled" onclick={save} disabled={store.saving.modules || !isDirty}>
    <svg viewBox="0 0 24 24" width="18" height="18"><path d={ICONS.save} fill="currentColor"/></svg>
    {store.saving.modules ? store.L.common.saving : store.L.modules.save}
  </button>
</BottomActions>