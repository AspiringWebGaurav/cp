import { invoke } from '@tauri-apps/api/core';
import type { SystemInfo, ClipboardFileInfo, ReleaseManifest } from '../types';

export const tauriApi = {
  getAppVersion: async (): Promise<string> => {
    return invoke<string>('get_app_version');
  },

  checkUpdate: async (currentVersion: string): Promise<ReleaseManifest | null> => {
    return invoke<ReleaseManifest | null>('check_update', { currentVersion });
  },

  downloadAndInstallUpdate: async (manifest: ReleaseManifest, pendingPaste?: boolean): Promise<string> => {
    return invoke<string>('download_and_install_update', { manifest, pendingPaste: pendingPaste ?? false });
  },

  checkPendingPaste: async (): Promise<boolean> => {
    return invoke<boolean>('check_and_consume_pending_paste');
  },

  getSystemInfo: async (): Promise<SystemInfo> => {
    return invoke<SystemInfo>('get_system_info');
  },

  getClipboardFiles: async (): Promise<ClipboardFileInfo | null> => {
    return invoke<ClipboardFileInfo | null>('get_clipboard_files');
  },

  executeCopyTransfer: async (sourcePath: string, destinationDir: string): Promise<SystemInfo> => {
    return invoke<SystemInfo>('execute_copy_transfer', {
      sourcePath,
      destinationDir,
    });
  },

  recordTransfer: async (bytes: number): Promise<SystemInfo> => {
    return invoke<SystemInfo>('record_transfer', { bytes });
  },

  resetStats: async (): Promise<SystemInfo> => {
    return invoke<SystemInfo>('reset_stats');
  },

  pauseTransfer: async (paused: boolean): Promise<void> => {
    return invoke<void>('pause_transfer', { paused });
  },

  cancelTransfer: async (): Promise<void> => {
    return invoke<void>('cancel_transfer');
  },

  hideWindow: async (): Promise<void> => {
    return invoke<void>('hide_window');
  },

  showWindow: async (): Promise<void> => {
    return invoke<void>('show_window');
  },

  isAutostartEnabled: async (): Promise<boolean> => {
    return invoke<boolean>('is_autostart_enabled');
  },

  setAutostartEnabled: async (enabled: boolean): Promise<boolean> => {
    return invoke<boolean>('set_autostart_enabled', { enabled });
  },
};

