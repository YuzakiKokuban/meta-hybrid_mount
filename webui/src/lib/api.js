import { exec } from 'kernelsu';
import { DEFAULT_CONFIG, PATHS } from './constants';

function serializeKvConfig(cfg) {
  const q = s => `"${s}"`;
  const lines = ['# Hybrid Mount Config', ''];
  lines.push(`moduledir = ${q(cfg.moduledir)}`);
  if (cfg.tempdir) lines.push(`tempdir = ${q(cfg.tempdir)}`);
  lines.push(`mountsource = ${q(cfg.mountsource)}`);
  lines.push(`verbose = ${cfg.verbose}`);
  lines.push(`force_ext4 = ${cfg.force_ext4}`);
  lines.push(`enable_nuke = ${cfg.enable_nuke}`);
  lines.push(`disable_umount = ${cfg.disable_umount}`);
  if (cfg.partitions.length) lines.push(`partitions = ${q(cfg.partitions.join(','))}`);
  return lines.join('\n');
}

export const API = {
  loadConfig: async () => {
    // Use centralized binary path
    const cmd = `${PATHS.BINARY} show-config`;
    try {
      const { errno, stdout } = await exec(cmd);
      if (errno === 0 && stdout) {
        return JSON.parse(stdout);
      } else {
        console.warn("Config load returned non-zero or empty, using defaults");
        return DEFAULT_CONFIG;
      }
    } catch (e) {
      console.error("Failed to load config from backend:", e);
      return DEFAULT_CONFIG; 
    }
  },

  saveConfig: async (config) => {
    const data = serializeKvConfig(config).replace(/'/g, "'\\''");
    const cmd = `mkdir -p "$(dirname "${PATHS.CONFIG}")" && printf '%s\n' '${data}' > "${PATHS.CONFIG}"`;
    const { errno } = await exec(cmd);
    if (errno !== 0) throw new Error('Failed to save config');
  },

  scanModules: async () => {
    const cmd = `${PATHS.BINARY} modules`;
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

  readLogs: async (logPath, lines = 1000) => {
    const f = logPath || DEFAULT_CONFIG.logfile;
    const cmd = `[ -f "${f}" ] && tail -n ${lines} "${f}" || echo ""`;
    const { errno, stdout, stderr } = await exec(cmd);
    
    if (errno === 0) return stdout || "";
    throw new Error(stderr || "Log file not found or unreadable");
  },

  getStorageUsage: async () => {
    try {
      const cmd = `${PATHS.BINARY} storage`;
      const { errno, stdout } = await exec(cmd);
      
      if (errno === 0 && stdout) {
        const data = JSON.parse(stdout);
        // Backend now returns the definitive type from the state file
        return {
          size: data.size || '-',
          used: data.used || '-',
          avail: data.avail || '-', 
          percent: data.percent || '0%',
          type: data.type || null
        };
      }
    } catch (e) {
      console.error("Storage check failed:", e);
    }
    return { size: '-', used: '-', percent: '0%', type: null };
  },

  getSystemInfo: async () => {
    try {
      // 1. Get static kernel/selinux info
      const cmdSys = `echo "KERNEL:$(uname -r)"; echo "SELINUX:$(getenforce)"`;
      const { errno: errSys, stdout: outSys } = await exec(cmdSys);
      
      let info = { kernel: '-', selinux: '-', mountBase: '-' };
      if (errSys === 0 && outSys) {
        outSys.split('\n').forEach(line => {
          if (line.startsWith('KERNEL:')) info.kernel = line.substring(7).trim();
          else if (line.startsWith('SELINUX:')) info.selinux = line.substring(8).trim();
        });
      }

      // 2. Read structured state JSON
      const cmdState = `cat "${PATHS.DAEMON_STATE}"`;
      const { errno: errState, stdout: outState } = await exec(cmdState);
      
      if (errState === 0 && outState) {
        try {
          const state = JSON.parse(outState);
          info.mountBase = state.mount_point || 'Unknown';
          // Potentially read other useful state here in the future
        } catch (e) {
          console.error("Failed to parse daemon state JSON", e);
        }
      }

      return info;
    } catch (e) {
      console.error("System info check failed:", e);
      return { kernel: 'Unknown', selinux: 'Unknown', mountBase: 'Unknown' };
    }
  },

  // Check active mounts filtered by mount source name
  getActiveMounts: async (sourceName) => {
    try {
      // 'mount' command lists all mounts. We grep for our source name.
      const src = sourceName || DEFAULT_CONFIG.mountsource;
      const cmd = `mount | grep "${src}"`; 
      const { errno, stdout } = await exec(cmd);
      
      const mountedParts = [];
      if (errno === 0 && stdout) {
        stdout.split('\n').forEach(line => {
          // Line format example: "KSU on /system type overlay ..."
          const parts = line.split(' ');
          if (parts.length >= 3 && parts[2].startsWith('/')) {
            const partName = parts[2].substring(1);
            if (partName) mountedParts.push(partName);
          }
        });
      }
      return mountedParts;
    } catch (e) {
      console.error("Mount check failed:", e);
      return [];
    }
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