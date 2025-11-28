import { API } from './api';
import { DEFAULT_CONFIG, DEFAULT_SEED } from './constants';
import { Monet } from './theme';
import locate from '../locate.json';

// Global state using Svelte 5 Runes
export const store = $state({
  config: { ...DEFAULT_CONFIG },
  modules: [],
  logs: [],
  storage: { used: '-', size: '-', percent: '0%' },
  
  // UI State
  loading: { config: false, modules: false, logs: false, status: false },
  saving: { config: false, modules: false },
  toast: { text: '', type: 'info', visible: false },
  
  // Settings
  theme: 'dark',
  lang: 'en',
  seed: DEFAULT_SEED,

  // Getters
  get L() {
    return locate[this.lang] || locate['en'];
  },

  get modeStats() {
    let auto = 0;
    let magic = 0;
    this.modules.forEach(m => {
      if (m.mode === 'magic') magic++;
      else auto++;
    });
    return { auto, magic };
  },

  // Actions
  showToast(msg, type = 'info') {
    this.toast = { text: msg, type, visible: true };
    setTimeout(() => { this.toast.visible = false; }, 3000);
  },

  setTheme(newTheme) {
    this.theme = newTheme;
    document.documentElement.setAttribute('data-theme', newTheme);
    localStorage.setItem('mm-theme', newTheme);
    Monet.apply(this.seed, newTheme === 'dark');
  },

  async init() {
    // Load local storage
    const savedLang = localStorage.getItem('mm-lang');
    if (savedLang && locate[savedLang]) this.lang = savedLang;
    
    const savedTheme = localStorage.getItem('mm-theme');
    const systemDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    this.setTheme(savedTheme || (systemDark ? 'dark' : 'light'));

    // Fetch system color
    const sysColor = await API.fetchSystemColor();
    if (sysColor) {
      this.seed = sysColor;
      Monet.apply(this.seed, this.theme === 'dark');
    }

    await this.loadConfig();
  },

  async loadConfig() {
    this.loading.config = true;
    try {
      this.config = await API.loadConfig();
      this.showToast(this.L.config.loadSuccess);
    } catch (e) {
      this.showToast(this.L.config.loadError, 'error');
    }
    this.loading.config = false;
  },

  async saveConfig() {
    this.saving.config = true;
    try {
      await API.saveConfig(this.config);
      this.showToast(this.L.config.saveSuccess);
    } catch (e) {
      this.showToast(this.L.config.saveFailed, 'error');
    }
    this.saving.config = false;
  },

  async loadModules() {
    this.loading.modules = true;
    this.modules = [];
    try {
      this.modules = await API.scanModules(this.config.moduledir);
    } catch (e) {
      this.showToast(this.L.modules.scanError, 'error');
    }
    this.loading.modules = false;
  },

  async saveModules() {
    this.saving.modules = true;
    try {
      await API.saveModules(this.modules);
      this.showToast(this.L.modules.saveSuccess);
    } catch (e) {
      this.showToast(this.L.modules.saveFailed, 'error');
    }
    this.saving.modules = false;
  },

  async loadLogs() {
    this.loading.logs = true;
    this.logs = [];
    try {
      const raw = await API.readLogs(this.config.logfile);
      this.logs = raw.split('\n').map(line => {
        let type = 'debug';
        if (line.includes('[ERROR]')) type = 'error';
        else if (line.includes('[WARN]')) type = 'warn';
        else if (line.includes('[INFO]')) type = 'info';
        return { text: line, type };
      });
    } catch (e) {
      this.logs = [{ text: this.L.logs.empty, type: 'debug' }];
    }
    this.loading.logs = false;
  },

  async loadStatus() {
    this.loading.status = true;
    // Load storage info and modules to calculate stats
    try {
      this.storage = await API.getStorageUsage();
      // We also need module count for the dashboard
      if (this.modules.length === 0) {
        this.modules = await API.scanModules(this.config.moduledir);
      }
    } catch (e) {
      // ignore
    }
    this.loading.status = false;
  }
});