<script>
  import { store } from '../lib/store.svelte';
  import { ICONS } from '../lib/constants';
  import locate from '../locate.json';
  
  import './NavBar.css';
  let { activeTab, onTabChange } = $props();
  let showLangMenu = $state(false);
  
  // Updated TABS to include 'status'
  const TABS = [
    { id: 'status', icon: ICONS.home },
    { id: 'config', icon: ICONS.settings },
    { id: 'modules', icon: ICONS.modules },
    { id: 'logs', icon: ICONS.description }
  ];
  
  const languages = Object.keys(locate).map(code => ({
    code,
    name: locate[code]?.lang?.display || code.toUpperCase()
  }));
  
  function toggleTheme() {
    store.setTheme(store.theme === 'light' ? 'dark' : 'light');
  }

  function setLang(code) {
    store.lang = code;
    showLangMenu = false;
    localStorage.setItem('mm-lang', code);
  }
</script>

<header class="app-bar">
  <div class="app-bar-content">
    <h1 class="screen-title">{store.L.common.appName}</h1>
    <div class="top-actions">
      <button class="btn-icon" onclick={toggleTheme} title={store.L.common.theme}>
        <svg viewBox="0 0 24 24"><path d={store.theme === 'light' ?
          ICONS.dark_mode : ICONS.light_mode} fill="currentColor"/></svg>
      </button>
      <button class="btn-icon" onclick={() => showLangMenu = !showLangMenu} title={store.L.common.language}>
        <svg viewBox="0 0 24 24"><path d={ICONS.translate} fill="currentColor"/></svg>
      </button>
    </div>
  </div>
  
  {#if showLangMenu}
    <div class="menu-dropdown">
      {#each languages as l}
        <button class="menu-item" onclick={() => setLang(l.code)}>{l.name}</button>
      {/each}
    </div>
  {/if}

  <nav class="nav-tabs">
    {#each TABS as tab}
      <button class="nav-tab {activeTab === tab.id ? 'active' : ''}" onclick={() => onTabChange(tab.id)}>
        <svg viewBox="0 0 24 24"><path d={tab.icon}/></svg>
        {store.L.tabs[tab.id]}
      </button>
    {/each}
  </nav>
</header>