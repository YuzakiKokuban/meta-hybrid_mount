<script lang="ts">
  import { store } from '../lib/store.svelte';
  import { ICONS } from '../lib/constants';
  import './TopBar.css';

  // State for the language dropdown menu
  let showLangMenu = $state(false);
  let langButtonRef = $state<HTMLButtonElement>();
  let menuRef = $state<HTMLDivElement>();

  /**
   * Toggles between light, dark, and auto themes.
   * Shows a toast notification upon change.
   */
  function toggleTheme() {
    let nextTheme: 'light' | 'dark' | 'auto';
    let toastMsg: string;
    const common = store.L?.common;

    if (store.theme === 'auto') {
      nextTheme = 'light';
      toastMsg = common?.themeLight ?? 'Light Mode';
    } else if (store.theme === 'light') {
      nextTheme = 'dark';
      toastMsg = common?.themeDark ?? 'Dark Mode';
    } else {
      nextTheme = 'auto';
      toastMsg = common?.themeAuto ?? 'Auto Mode';
    }

    store.setTheme(nextTheme);
    store.showToast(toastMsg, 'info');
  }

  /**
   * Returns the SVG path for the current theme icon.
   */
  function getThemeIcon() {
    if (store.theme === 'auto') return ICONS.auto_mode;
    if (store.theme === 'light') return ICONS.light_mode;
    return ICONS.dark_mode;
  }

  /**
   * Sets the application language and closes the menu.
   */
  function setLang(code: string) {
    store.setLang(code);
    showLangMenu = false;
  }
  
  /**
   * Closes the language menu if clicked outside.
   */
  function handleOutsideClick(e: MouseEvent) {
    if (showLangMenu && 
        menuRef && !menuRef.contains(e.target as Node) && 
        langButtonRef && !langButtonRef.contains(e.target as Node)) {
      showLangMenu = false;
    }
  }
</script>

<svelte:window onclick={handleOutsideClick} />

<header class="top-bar">
  <div class="top-bar-content">
    <h1 class="screen-title">{store.L?.common?.appName}</h1>
    <div class="top-actions">
      <button class="btn-icon" onclick={toggleTheme} title={store.L?.common?.theme}>
        <svg viewBox="0 0 24 24"><path d={getThemeIcon()} fill="currentColor"/></svg>
      </button>

      <button 
        class="btn-icon" 
        bind:this={langButtonRef}
        onclick={() => showLangMenu = !showLangMenu} 
        title={store.L?.common?.language}
      >
        <svg viewBox="0 0 24 24"><path d={ICONS.translate} fill="currentColor"/></svg>
      </button>
    </div>
  </div>
  
  {#if showLangMenu}
    <div class="menu-dropdown" bind:this={menuRef}>
      {#each store.availableLanguages ?? [] as l}
        <button class="menu-item" onclick={() => setLang(l.code)}>{l.name}</button>
      {/each}
    </div>
  {/if}
</header>