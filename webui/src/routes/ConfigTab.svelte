<script lang="ts">
  import { store } from '../lib/store.svelte';
  import { ICONS, DEFAULT_CONFIG } from '../lib/constants';
  import ChipInput from '../components/ChipInput.svelte';
  import { slide } from 'svelte/transition';
  import './ConfigTab.css';

  let initialConfigStr = $state('');

  const isValidPath = (p: string) => !p || (p.startsWith('/') && p.length > 1);
  let invalidModuleDir = $derived(!isValidPath(store.config.moduledir));
  let invalidTempDir = $derived(store.config.tempdir && !isValidPath(store.config.tempdir));
  let isDirty = $derived.by(() => {
    if (!initialConfigStr) return false;
    return JSON.stringify(store.config) !== initialConfigStr;
  });
  $effect(() => {
    if (!store.loading.config && store.config) {
      if (!initialConfigStr || initialConfigStr === JSON.stringify(DEFAULT_CONFIG)) {
        initialConfigStr = JSON.stringify(store.config);
      }
    }
  });
  function save() {
    if (invalidModuleDir || invalidTempDir) {
      store.showToast(store.L.config.invalidPath, "error");
      return;
    }
    store.saveConfig().then(() => {
        initialConfigStr = JSON.stringify(store.config);
    });
  }
  
  function reload() {
    store.loadConfig().then(() => {
        initialConfigStr = JSON.stringify(store.config);
    });
  }

  function resetTempDir() {
    store.config.tempdir = "";
  }
</script>

<div class="md3-card">
  <div class="switch-row">
    <span>{store.L.config.verboseLabel}</span>
    <label class="md3-switch">
      <input type="checkbox" bind:checked={store.config.verbose}>
      <span class="track"><span class="thumb"></span></span>
    </label>
  </div>

  {#if store.config.verbose}
    <div class="switch-row" transition:slide>
      <span>{store.L.config.dryRun}</span>
      <label class="md3-switch">
        <input type="checkbox" bind:checked={store.config.dry_run}>
        <span class="track"><span class="thumb"></span></span>
      </label>
    </div>
  {/if}

  <div class="switch-row">
    <span>{store.L.config.forceExt4}</span>
    <label class="md3-switch">
      <input type="checkbox" bind:checked={store.config.force_ext4}>
      <span class="track"><span class="thumb"></span></span>
    </label>
  </div>
  <div class="switch-row">
    <span>{store.L.config.enableNuke}</span>
    <label class="md3-switch">
      <input type="checkbox" bind:checked={store.config.enable_nuke}>
      <span class="track"><span class="thumb"></span></span>
    </label>
  </div>
  <div class="switch-row">
    <span>{store.L.config.disableUmount}</span>
    <label class="md3-switch">
      <input type="checkbox" bind:checked={store.config.disable_umount}>
      <span class="track"><span class="thumb"></span></span>
    </label>
  </div>
</div>

<div class="md3-card">
  <div class="text-field" class:error={invalidModuleDir}>
    <input type="text" id="c-moduledir" bind:value={store.config.moduledir} placeholder={DEFAULT_CONFIG.moduledir} />
    <label for="c-moduledir">{store.L.config.moduleDir}</label>
  </div>
  
  <div class="text-field" class:error={invalidTempDir} style="display:flex; align-items:center;">
    <input type="text" id="c-tempdir" bind:value={store.config.tempdir} placeholder={store.L.config.autoPlaceholder} />
    <label for="c-tempdir">{store.L.config.tempDir}</label>
    
    {#if store.config.tempdir}
      <button class="icon-reset" onclick={resetTempDir} title={store.L.config.reset}>
        ✕
      </button>
    {/if}
  </div>
  
  <div class="text-field">
    <input type="text" id="c-mountsource" bind:value={store.config.mountsource} placeholder={DEFAULT_CONFIG.mountsource} />
    <label for="c-mountsource">{store.L.config.mountSource}</label>
  </div>
  
  <div style="position: relative; margin-top: 4px;">
    <span style="font-size: 12px; color: var(--md-sys-color-primary); position: absolute; top: -9px; left: 12px; background: var(--md-sys-color-surface-container); padding: 0 4px; z-index: 1;">
      {store.L.config.partitions}
    </span>
    <ChipInput bind:values={store.config.partitions} placeholder="mi_ext, product..." />
  </div>
</div>

<div class="bottom-actions">
  <button 
    class="btn-tonal" 
    onclick={reload}
    disabled={store.loading.config}
    title={store.L.config.reload}
  >
    <svg viewBox="0 0 24 24" width="20" height="20"><path d={ICONS.refresh} fill="currentColor"/></svg>
  </button>
  <button class="btn-filled" onclick={save} disabled={store.saving.config || !isDirty}>
    <svg viewBox="0 0 24 24" width="18" height="18"><path d={ICONS.save} fill="currentColor"/></svg>
    {store.saving.config ? store.L.common.saving : store.L.config.save}
  </button>
</div>