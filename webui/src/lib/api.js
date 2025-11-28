import { exec } from 'kernelsu';
import { DEFAULT_CONFIG, PATHS } from './constants';

function parseKvConfig(text) {
  try {
    const result = { ...DEFAULT_CONFIG };
    text.split('\n').forEach(line => {
      line = line.trim();
      if (!line || line.startsWith('#')) return;
      
      const eq = line.indexOf('=');
      if (eq < 0) return;
      
      let key = line.slice(0, eq).trim();
      let val = line.slice(eq + 1).trim();
      
      if ((val.startsWith('"') && val.endsWith('"')) || (val.startsWith("'") && val.endsWith("'"))) {
        val = val.slice(1, -1);
      }
      
      if (key === 'moduledir') result.moduledir = val;
      else if (key === 'tempdir') result.tempdir = val;
      else if (key === 'mountsource') result.mountsource = val;
      else if (key === 'verbose') result.verbose = (val === 'true');
      else if (key === 'partitions') result.partitions = val.split(',').map(s => s.trim()).filter(Boolean);
    });
    return result;
  } catch (e) { return null; }
}

function serializeKvConfig(cfg) {
  const q = s => `"${s}"`;
  const lines = ['# Hybrid Mount Config', ''];
  lines.push(`moduledir = ${q(cfg.moduledir)}`);
  if (cfg.tempdir) lines.push(`tempdir = ${q(cfg.tempdir)}`);
  lines.push(`mountsource = ${q(cfg.mountsource)}`);
  lines.push(`verbose = ${cfg.verbose}`);
  if (cfg.partitions.length) lines.push(`partitions = ${q(cfg.partitions.join(','))}`);
  return lines.join('\n');
}

export const API = {
  loadConfig: async () => {
    const { errno, stdout } = await exec(`[ -f "${PATHS.CONFIG}" ] && cat "${PATHS.CONFIG}" || echo ""`);
    if (errno !== 0) throw new Error('Failed to load config');
    return (stdout.trim()) ? (parseKvConfig(stdout) || DEFAULT_CONFIG) : DEFAULT_CONFIG;
  },

  saveConfig: async (config) => {
    const data = serializeKvConfig(config).replace(/'/g, "'\\''");
    const cmd = `mkdir -p "$(dirname "${PATHS.CONFIG}")" && printf '%s\n' '${data}' > "${PATHS.CONFIG}"`;
    const { errno } = await exec(cmd);
    if (errno !== 0) throw new Error('Failed to save config');
  },

  scanModules: async () => {
    // Execute backend binary to get JSON list of modules
    // Backend also handles mode config reading now!
    const cmd = "/data/adb/modules/meta-hybrid/meta-hybrid modules";
    try {
      const { errno, stdout } = await exec(cmd);
      if (errno === 0 && stdout) {
        return JSON.parse(stdout);
      }
    } catch (e) {
      console.error("Module scan failed:", e);
    }
    return [];
  },

  saveModules: async (modules) => {
    let content = "# Module Modes\n";
    modules.forEach(m => { if (m.mode !== 'auto') content += `${m.id}=${m.mode}\n`; });
    const data = content.replace(/'/g, "'\\''");
    const { errno } = await exec(`mkdir -p "$(dirname "${PATHS.MODE_CONFIG}")" && printf '%s\n' '${data}' > "${PATHS.MODE_CONFIG}"`);
    if (errno !== 0) throw new Error('Failed to save modes');
  },

  readLogs: async (logPath) => {
    const f = logPath || DEFAULT_CONFIG.logfile;
    const { errno, stdout, stderr } = await exec(`[ -f "${f}" ] && cat "${f}" || echo ""`);
    if (errno === 0 && stdout) return stdout;
    throw new Error(stdout || stderr || "Log file empty or not found");
  },

  getStorageUsage: async () => {
    try {
      const cmd = "/data/adb/modules/meta-hybrid/meta-hybrid storage";
      const { errno, stdout } = await exec(cmd);
      
      if (errno === 0 && stdout) {
        const data = JSON.parse(stdout);
        return {
          size: data.size || '-',
          used: data.used || '-',
          avail: data.avail || '-', 
          percent: data.percent || '0%'
        };
      }
    } catch (e) {
      console.error("Storage check failed:", e);
    }
    return { size: '-', used: '-', percent: '0%' };
  },

  fetchSystemColor: async () => {
    try {
      const { stdout } = await exec('settings get secure theme_customization_overlay_packages');
      if (stdout) {
        const match = /["']?android\.theme\.customization\.system_palette["']?\s*:\s*["']?#?([0-9a-fA-F]{6,8})["']?/i.exec(stdout) || 
                      /["']?source_color["']?\s*:\s*["']?#?([0-9a-fA-F]{6,8})["']?/i.exec(stdout);
        if (match && match[1]) {
          let hex = match[1];
          if (hex.length === 8) hex = hex.substring(2);
          return '#' + hex;
        }
      }
    } catch (e) {}
    return null;
  }
};