import { useState, useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { Header } from './components/Header';
import { StatusGrid } from './components/StatusGrid';
import { TransferView } from './components/TransferView';
import { UpdateView, type UpdateUIState, type UpdateTriggerReason } from './components/UpdateView';
import { tauriApi } from './services/tauriApi';
import type { SystemInfo, ClipboardFileInfo, TransferState, ProgressUpdate, ReleaseManifest, UpdateDownloadProgress } from './types';

export default function App() {
  const [currentVersion, setCurrentVersion] = useState<string>('0.0.1');
  const [systemInfo, setSystemInfo] = useState<SystemInfo>({
    computerName: 'NAMITA',
    status: 'ON (intercepting)',
    hardware: 'NVMe SSD',
    engineVersion: '0.0.1',
    transfersCompleted: 0,
    volumeMoved: '0 B',
    speedRating: 'PCIe 3.0 / 3.4 GB/s',
    processor: 'Intel Core Processor',
    memory: '8 GB RAM',
  });

  const [clipboardFiles, setClipboardFiles] = useState<ClipboardFileInfo | null>(null);

  const [transfer, setTransfer] = useState<TransferState>({
    isActive: false,
    isPaused: false,
    source: '',
    destination: '',
    currentFile: '',
    progress: 0,
    speed: '3.4 GB/s',
    bytesTransferred: '0 B',
    totalBytes: '0 B',
    eta: '0s',
  });

  const [activeTab, setActiveTab] = useState<'status' | 'transfer' | 'update'>('status');

  // Update State Machine
  const [updateManifest, setUpdateManifest] = useState<ReleaseManifest | null>(null);
  const [updateState, setUpdateState] = useState<UpdateUIState>('checking');
  const [updateReason, setUpdateReason] = useState<UpdateTriggerReason>('startup');
  const [updateProgress, setUpdateProgress] = useState<UpdateDownloadProgress | null>(null);
  const [updateError, setUpdateError] = useState<string | null>(null);
  const [isCheckingUpdate, setIsCheckingUpdate] = useState(false);

  // Synchronous State Refs to prevent stale closures in global event hooks
  const updateManifestRef = useRef<ReleaseManifest | null>(null);
  const updateStateRef = useRef<UpdateUIState>('checking');
  const updateReasonRef = useRef<UpdateTriggerReason>('startup');
  const isUpdatingInProgress = useRef(false);
  const userDismissedUpdate = useRef(false);
  const lastUpdateCheckTime = useRef(0);
  const isPastingInProgressRef = useRef(false);
  const lastPasteTimeRef = useRef(0);

  useEffect(() => {
    updateManifestRef.current = updateManifest;
    updateStateRef.current = updateState;
    updateReasonRef.current = updateReason;
  }, [updateManifest, updateState, updateReason]);

  const handleStartUpdate = async (targetManifest?: ReleaseManifest) => {
    const target = targetManifest || updateManifestRef.current;
    if (!target || isUpdatingInProgress.current) return;
    isUpdatingInProgress.current = true;
    const isPendingPaste = updateReasonRef.current === 'copy';
    setUpdateState('downloading');
    setUpdateError(null);
    try {
      await tauriApi.downloadAndInstallUpdate(target, isPendingPaste);
      setUpdateState('restarting');
    } catch (err: any) {
      console.error('Update failed:', err);
      setUpdateError(err?.message || String(err));
      setUpdateState('error');
    } finally {
      isUpdatingInProgress.current = false;
    }
  };

  const refreshTelemetry = async () => {
    try {
      const [info, ver] = await Promise.all([
        tauriApi.getSystemInfo().catch(() => null),
        tauriApi.getAppVersion().catch(() => null),
      ]);
      if (info) {
        setSystemInfo(info);
      }
      if (ver) {
        setCurrentVersion(ver);
        setSystemInfo((prev) => ({ ...prev, engineVersion: ver }));
      }
    } catch (e) {
      console.warn('Telemetry refresh warning:', e);
    }
  };

  const checkForUpdates = async (reason: UpdateTriggerReason = 'landing', force = false) => {
    const shouldForce = force || reason === 'landing' || reason === 'manual' || reason === 'startup';
    const now = Date.now();
    if (!shouldForce && now - lastUpdateCheckTime.current < 2500) {
      return;
    }
    lastUpdateCheckTime.current = now;

    setIsCheckingUpdate(true);
    const checkStartTime = Date.now();

    try {
      const ver = await tauriApi.getAppVersion().catch(() => currentVersion);
      setCurrentVersion(ver);
      setSystemInfo((prev) => ({ ...prev, engineVersion: ver }));

      const manifest = await tauriApi.checkUpdate(ver);

      // Keep minimum 600ms visual spin so the user has crisp, clear feedback that GPHost was queried
      const elapsed = Date.now() - checkStartTime;
      if (elapsed < 600) {
        await new Promise((r) => setTimeout(r, 600 - elapsed));
      }

      if (manifest && manifest.version) {
        setUpdateManifest(manifest);
        setUpdateReason(reason);
        setUpdateState('available');

        // Automatically trigger update when:
        // 1. Forced check (e.g. user clicked Update badge)
        // 2. Manual check
        // 3. Intercepted Copy/Paste
        // 4. User opened the landing UI (like after install, or from tray) unless user explicitly dismissed it this session
        const shouldAutoUpdate =
          force ||
          reason === 'manual' ||
          reason === 'copy' ||
          ((reason === 'startup' || reason === 'landing') && !userDismissedUpdate.current);

        if (shouldAutoUpdate) {
          setActiveTab('update');
          await tauriApi.showWindow().catch(() => {});
          handleStartUpdate(manifest);
        }
      } else {
        setUpdateManifest(null);
        setUpdateState('checking');
        if (reason === 'manual' || reason === 'landing') {
          setActiveTab('status');
        }
      }
    } catch (err: any) {
      console.warn('Update check note:', err);
      const elapsed = Date.now() - checkStartTime;
      if (elapsed < 600) {
        await new Promise((r) => setTimeout(r, 600 - elapsed));
      }
      if (reason === 'manual' || reason === 'copy') {
        setUpdateError(err?.message || String(err));
        setUpdateState('error');
        setActiveTab('update');
      }
    } finally {
      setIsCheckingUpdate(false);
    }
  };

  const refreshLandingUI = async (reason: UpdateTriggerReason = 'landing', force = false) => {
    await refreshTelemetry();

    const isUpdatingNow =
      updateStateRef.current === 'downloading' ||
      updateStateRef.current === 'verifying' ||
      updateStateRef.current === 'ready' ||
      updateStateRef.current === 'restarting';

    if (isUpdatingNow) {
      setActiveTab('update');
      return;
    }

    await checkForUpdates(reason, force);
  };

  useEffect(() => {
    // Initial refresh of telemetry and auto-update check on landing UI open
    refreshLandingUI('startup');

    // Interval for real-time telemetry metrics
    const interval = setInterval(refreshTelemetry, 2000);

    // Periodic background update check every 60 seconds
    const bgUpdateInterval = setInterval(() => {
      checkForUpdates('startup');
    }, 60000);

    // Check if app just restarted from a seamless self-update while user was copying/pasting
    tauriApi.checkPendingPaste()
      .then(async (hasPending) => {
        if (hasPending) {
          const clip = await tauriApi.getClipboardFiles().catch(() => null);
          if (clip && clip.filePaths.length > 0) {
            setClipboardFiles(clip);
            handlePasteWithCP(clip);
          }
        }
      })
      .catch(() => {});

    let unlistenStats: (() => void) | undefined;
    listen('stats-reset', () => {
      refreshTelemetry();
    }).then((u) => { unlistenStats = u; }).catch(() => {});

    let unlistenAutoPaste: (() => void) | undefined;
    listen('trigger-auto-paste', async () => {
      try {
        const now = Date.now();
        if (isPastingInProgressRef.current || now - lastPasteTimeRef.current < 1200) {
          return;
        }

        const isUpdatingNow =
          updateStateRef.current === 'downloading' ||
          updateStateRef.current === 'verifying' ||
          updateStateRef.current === 'ready' ||
          updateStateRef.current === 'restarting';

        // If an update is in progress or available, hold the user on the update screen!
        if (isUpdatingNow || updateManifestRef.current) {
          setUpdateReason('copy');
          setActiveTab('update');
          await tauriApi.showWindow().catch(() => {});
          if (updateManifestRef.current && !isUpdatingNow && updateStateRef.current !== 'error') {
            handleStartUpdate(updateManifestRef.current);
          }
          return;
        }

        const clip = await tauriApi.getClipboardFiles();
        if (clip && clip.filePaths.length > 0) {
          setClipboardFiles(clip);
          handlePasteWithCP(clip);
        }
      } catch (e) {
        console.error('Auto paste failed:', e);
      }
    }).then((u) => { unlistenAutoPaste = u; }).catch(() => {});

    // Refresh landing UI whenever console window is opened or shown from tray
    let unlistenShowConsole: (() => void) | undefined;
    listen('show-console', () => {
      refreshLandingUI('landing', true);
    }).then((u) => { unlistenShowConsole = u; }).catch(() => {});

    // Refresh landing UI whenever window regains user focus
    const handleWindowFocus = () => {
      refreshLandingUI('landing', true);
    };
    window.addEventListener('focus', handleWindowFocus);

    // Refresh landing UI whenever document becomes visible
    const handleVisibility = () => {
      if (document.visibilityState === 'visible') {
        refreshLandingUI('landing', true);
      }
    };
    document.addEventListener('visibilitychange', handleVisibility);

    let unlistenProgress: (() => void) | undefined;
    listen<ProgressUpdate>('transfer-progress', (event) => {
      const data = event.payload;
      setTransfer((prev) => ({
        ...prev,
        progress: data.percentage,
        bytesTransferred: data.formattedCopied,
        totalBytes: data.formattedTotal,
        speed: data.speed,
        eta: data.isFinished ? 'Complete!' : data.eta,
        isActive: !data.isFinished,
      }));
    }).then((u) => { unlistenProgress = u; }).catch(() => {});

    // Listen to real-time update download progress events from Rust
    let unlistenUpdateProgress: (() => void) | undefined;
    listen<UpdateDownloadProgress>('update-download-progress', (event) => {
      const data = event.payload;
      setUpdateProgress(data);
      if (data.stage === 'downloading') {
        setUpdateState('downloading');
      } else if (data.stage === 'verifying') {
        setUpdateState('verifying');
      } else if (data.stage === 'ready') {
        setUpdateState('ready');
      }
    }).then((u) => { unlistenUpdateProgress = u; }).catch(() => {});

    return () => {
      clearInterval(interval);
      clearInterval(bgUpdateInterval);
      window.removeEventListener('focus', handleWindowFocus);
      document.removeEventListener('visibilitychange', handleVisibility);
      if (unlistenStats) unlistenStats();
      if (unlistenAutoPaste) unlistenAutoPaste();
      if (unlistenShowConsole) unlistenShowConsole();
      if (unlistenProgress) unlistenProgress();
      if (unlistenUpdateProgress) unlistenUpdateProgress();
    };
  }, []);

  const handlePasteWithCP = async (info?: ClipboardFileInfo) => {
    const now = Date.now();
    if (isPastingInProgressRef.current || now - lastPasteTimeRef.current < 1200) {
      return;
    }
    isPastingInProgressRef.current = true;
    lastPasteTimeRef.current = now;

    const isUpdatingNow =
      updateStateRef.current === 'downloading' ||
      updateStateRef.current === 'verifying' ||
      updateStateRef.current === 'ready' ||
      updateStateRef.current === 'restarting';

    // Hold user if update is active or available
    if (isUpdatingNow || updateManifestRef.current) {
      setUpdateReason('copy');
      setActiveTab('update');
      await tauriApi.showWindow().catch(() => {});
      if (updateManifestRef.current && !isUpdatingNow && updateStateRef.current !== 'error') {
        handleStartUpdate(updateManifestRef.current);
      }
      isPastingInProgressRef.current = false;
      return;
    }

    const target = info || clipboardFiles;
    if (!target || target.filePaths.length === 0) {
      isPastingInProgressRef.current = false;
      return;
    }

    const destDir = target.activeExplorerPath || 'D:\\';
    setActiveTab('transfer');

    try {
      for (let i = 0; i < target.filePaths.length; i++) {
        const src = target.filePaths[i];
        const fileName = src.split(/[/\\]/).pop() || target.primaryName;

        setTransfer({
          isActive: true,
          isPaused: false,
          source: src,
          destination: destDir,
          currentFile: target.filePaths.length > 1 ? `(${i + 1}/${target.filePaths.length}) ${fileName}` : fileName,
          progress: 0,
          speed: 'Starting...',
          bytesTransferred: '0 B',
          totalBytes: target.formattedSize,
          eta: 'Calculating...',
        });

        const updated = await tauriApi.executeCopyTransfer(src, destDir);
        setSystemInfo(updated);
      }

      setTransfer((prev) => ({
        ...prev,
        progress: 100,
        bytesTransferred: target.formattedSize,
        speed: '3.4 GB/s',
        eta: 'Complete!',
        isActive: false,
      }));

      setTimeout(async () => {
        await tauriApi.hideWindow().catch(() => {});
        setActiveTab('status');
      }, 1000);
    } catch (err) {
      console.warn('Transfer completed, canceled, or stopped:', err);
      setTransfer((prev) => ({ ...prev, isActive: false, progress: 0 }));
      await tauriApi.hideWindow().catch(() => {});
      setActiveTab('status');
      refreshTelemetry();
    } finally {
      isPastingInProgressRef.current = false;
    }
  };

  const handleTogglePause = async () => {
    const nextState = !transfer.isPaused;
    setTransfer((p) => ({ ...p, isPaused: nextState }));
    await tauriApi.pauseTransfer(nextState).catch(() => {});
  };

  const handleCancelTransfer = async () => {
    await tauriApi.cancelTransfer().catch(() => {});
    setTransfer((p) => ({ ...p, isActive: false, progress: 0 }));
    await tauriApi.hideWindow().catch(() => {});
    setActiveTab('status');
    refreshTelemetry();
  };

  return (
    <div className="w-full h-full bg-white text-zinc-900 select-none flex flex-col justify-between p-3.5 text-[12px] font-sans antialiased overflow-hidden">
      {/* 1. Header Lockup */}
      <Header 
        currentVersion={currentVersion}
        hasUpdate={Boolean(updateManifest)}
        targetVersion={updateManifest?.version}
        isChecking={isCheckingUpdate}
        onOpenUpdate={() => {
          userDismissedUpdate.current = false;
          setUpdateReason('manual');
          setActiveTab('update');
          if (updateManifestRef.current) {
            handleStartUpdate(updateManifestRef.current);
          } else {
            checkForUpdates('manual', true);
          }
        }}
        onCheckUpdate={() => {
          userDismissedUpdate.current = false;
          refreshLandingUI('manual', true);
        }}
      />

      {/* 2. Middle Content Area */}
      {activeTab === 'status' ? (
        <StatusGrid 
          systemInfo={systemInfo} 
          hasUpdate={Boolean(updateManifest)}
          targetVersion={updateManifest?.version}
          onOpenUpdate={() => {
            userDismissedUpdate.current = false;
            setUpdateReason('manual');
            setActiveTab('update');
            if (updateManifestRef.current) {
              handleStartUpdate(updateManifestRef.current);
            } else {
              checkForUpdates('manual', true);
            }
          }}
        />
      ) : activeTab === 'transfer' ? (
        <TransferView 
          transfer={transfer} 
          onTogglePause={handleTogglePause} 
          onCancel={handleCancelTransfer} 
        />
      ) : (
        <UpdateView
          currentVersion={currentVersion}
          manifest={updateManifest}
          state={updateState}
          reason={updateReason}
          progress={updateProgress}
          errorMessage={updateError}
          onStartUpdate={() => {
            userDismissedUpdate.current = false;
            handleStartUpdate();
          }}
          onCancel={() => {
            userDismissedUpdate.current = true;
            setActiveTab('status');
          }}
          onRetry={() => {
            userDismissedUpdate.current = false;
            handleStartUpdate();
          }}
        />
      )}
    </div>
  );
}
