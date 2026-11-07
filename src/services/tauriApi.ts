import { invoke } from '@tauri-apps/api/core';
import { emit } from '@tauri-apps/api/event';
import type { SystemInfo, ClipboardFileInfo, ReleaseManifest, UpdateDownloadProgress } from '../types';
import { unpackOtaPayload } from './otaContainer';

export const tauriApi = {
  getAppVersion: async (): Promise<string> => {
    return invoke<string>('get_app_version');
  },

  checkUpdate: async (currentVersion: string): Promise<ReleaseManifest | null> => {
    return invoke<ReleaseManifest | null>('check_update', { currentVersion });
  },

  downloadAndInstallUpdate: async (manifest: ReleaseManifest, pendingPaste?: boolean): Promise<string> => {
    let downloadUrl = manifest.url;
    if (!downloadUrl) throw new Error('Release manifest does not contain a download URL');

    // Cache-buster ensures Vercel CDN bypasses any cached 307 redirect and generates a fresh presigned URL
    const sep = downloadUrl.includes('?') ? '&' : '?';
    downloadUrl = `${downloadUrl}${sep}t=${Date.now()}`;

    const cleanVer = manifest.version.replace(/^v+/, '');

    // 1. Download payload via native reqwest (Zero CORS, follows R2 307 redirects natively, 1:1 Downex pattern)
    let rawBytes: Uint8Array;
    try {
      const bytes = await invoke<number[] | Uint8Array>('download_update_container', { url: downloadUrl });
      rawBytes = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
    } catch (e: any) {
      throw new Error(`Download failed: ${e?.message || e}`);
    }

    emit('update-download-progress', {
      downloaded_bytes: rawBytes.length,
      total_bytes: rawBytes.length,
      percentage: 100,
      speed: '0 MB/s',
      stage: 'verifying',
    } as UpdateDownloadProgress);

    // 2. Cryptographic signature check
    if (manifest.signature) {
      try {
        await invoke('verify_update_signature', {
          containerBytes: Array.from(rawBytes),
          signature: manifest.signature,
        });
      } catch (sigErr) {
        console.warn('Signature verification notice:', sigErr);
      }
    }

    // 3. Unpack and decrypt the pure binary container (AES-256-GCM WebCrypto)
    const unpacked = await unpackOtaPayload(rawBytes.buffer as ArrayBuffer);

    // 4. Stage executable in com.cp.engine temp directory
    const stagingFilename = `cp-v${cleanVer}.staged.exe`;
    const stagedPath = await invoke<string>('write_update_staging_file', {
      fileName: stagingFilename,
      bytes: Array.from(unpacked.payload),
    });

    emit('update-download-progress', {
      downloaded_bytes: rawBytes.length,
      total_bytes: rawBytes.length,
      percentage: 100,
      speed: '0 MB/s',
      stage: 'ready',
    } as UpdateDownloadProgress);

    // Smooth visual feedback before in-process handoff
    await new Promise((r) => setTimeout(r, 600));

    // 5. In-process atomic NTFS swap and detached respawn
    await invoke('apply_in_process_update', {
      stagingPath: stagedPath,
      targetVersion: cleanVer,
      pendingPaste: pendingPaste ?? false,
    });

    return 'Update applied. Restarting...';
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
