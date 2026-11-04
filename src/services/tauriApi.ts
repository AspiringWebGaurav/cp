import { invoke } from '@tauri-apps/api/core';
import type { SystemInfo, ClipboardFileInfo } from '../types';

export const tauriApi = {
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
};

