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
    computerName: 'GAURAV',
    status: 'ON (intercepting)',
    hardware: 'NVMe SSD + SATA HDD',
    engineVersion: '0.0.1',
    transfersCompleted: 0,
    volumeMoved: '0 B',
    speedRating: 'PCIe 3.0 / 3.4 GB/s',
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

  useEffect(() => {
    updateManifestRef.current = updateManifest;
    updateStateRef.current = updateState;
    updateReasonRef.current = updateReason;
  }, [updateManifest, updateState, updateReason]);

  const handleStartUpdate = async (targetManifest?: ReleaseManifest) => {
    const target = targetManifest || updateManifestRef.current;
    if (!target) return;
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
    }
  };

  const checkForUpdates = async (reason: UpdateTriggerReason = 'startup') => {
    setIsCheckingUpdate(true);
    try {
      const ver = await tauriApi.getAppVersion().catch(() => '0.0.1');
      setCurrentVersion(ver);
      setSystemInfo((prev) => ({ ...prev, engineVersion: ver }));

      const manifest = await tauriApi.checkUpdate(ver);
      if (manifest && manifest.version) {
        setUpdateManifest(manifest);
        setUpdateReason(reason);
        // Only show window and start update immediately if explicitly requested (manual check or copy)
        // Background startup check stays completely silent in system tray so copy/paste interception can be tested!
        if (reason === 'manual' || reason === 'copy') {
          setActiveTab('update');
          await tauriApi.showWindow().catch(() => {});
          handleStartUpdate(manifest);
        }
      } else {
        setUpdateManifest(null);
        if (reason === 'manual') {
          setActiveTab('status');
        }
      }
    } catch (err: any) {
      if (reason === 'manual') {
        setUpdateError(err?.message || String(err));
        setUpdateState('error');
        setActiveTab('update');
      }
    } finally {
      setIsCheckingUpdate(false);
    }
  };

  useEffect(() => {
    const refreshTelemetry = () => {
      tauriApi.getSystemInfo()
        .then((info) => {
          setSystemInfo(info);
          if (info.engineVersion) {
            setCurrentVersion(info.engineVersion);
          }
        })
        .catch(() => {});
    };

    refreshTelemetry();
    const interval = setInterval(refreshTelemetry, 2000);

    // Initial autonomous update check on application startup
    checkForUpdates('startup');

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

    let unlistenShowConsole: (() => void) | undefined;
    listen('show-console', () => {
      const isUpdatingNow =
        updateStateRef.current === 'downloading' ||
        updateStateRef.current === 'verifying' ||
        updateStateRef.current === 'ready' ||
        updateStateRef.current === 'restarting';
      if (isUpdatingNow || updateManifestRef.current) {
        setActiveTab('update');
      } else {
        setActiveTab('status');
      }
    }).then((u) => { unlistenShowConsole = u; }).catch(() => {});

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

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key.toLowerCase() === 'v') {
        const isUpdatingNow =
          updateStateRef.current === 'downloading' ||
          updateStateRef.current === 'verifying' ||
          updateStateRef.current === 'ready' ||
          updateStateRef.current === 'restarting';
        if (isUpdatingNow || updateManifestRef.current) {
          setUpdateReason('copy');
          setActiveTab('update');
          return;
        }
        handlePasteWithCP();
      } else if (e.key.toLowerCase() === 't') {
        startBenchmarkTransfer();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => {
      clearInterval(interval);
      if (unlistenStats) unlistenStats();
      if (unlistenAutoPaste) unlistenAutoPaste();
      if (unlistenShowConsole) unlistenShowConsole();
      if (unlistenProgress) unlistenProgress();
      if (unlistenUpdateProgress) unlistenUpdateProgress();
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, []);

  const handlePasteWithCP = async (info?: ClipboardFileInfo) => {
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
      return;
    }

    const target = info || clipboardFiles;
    if (!target || target.filePaths.length === 0) return;

    const destDir = target.activeExplorerPath || 'D:\\';
    const src = target.filePaths[0];

    setActiveTab('transfer');
    setTransfer({
      isActive: true,
      isPaused: false,
      source: src,
      destination: destDir,
      currentFile: target.primaryName,
      progress: 0,
      speed: 'Starting...',
      bytesTransferred: '0 B',
      totalBytes: target.formattedSize,
      eta: 'Calculating...',
    });

    try {
      const updated = await tauriApi.executeCopyTransfer(src, destDir);
      setSystemInfo(updated);
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
      console.error('Transfer failed or cancelled:', err);
      await tauriApi.hideWindow().catch(() => {});
      setActiveTab('status');
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
  };

  const startBenchmarkTransfer = () => {
    setActiveTab('transfer');
    setTransfer({
      isActive: true,
      isPaused: false,
      source: 'C:\\Downloads\\Ubuntu-24.04-LTS-Desktop-x86_64.iso',
      destination: 'D:\\',
      currentFile: 'Ubuntu-24.04-LTS-Desktop-x86_64.iso',
      progress: 0,
      speed: '3.4 GB/s',
      bytesTransferred: '0.0 GB',
      totalBytes: '8.25 GB',
      eta: '2.4s',
    });

    const interval = setInterval(() => {
      setTransfer((prev) => {
        if (prev.isPaused) return prev;
        const next = Math.min(100, prev.progress + 10);
        const transferred = ((next / 100) * 8.25).toFixed(2);
        if (next >= 100) {
          clearInterval(interval);
          tauriApi.recordTransfer(8858370048)
            .then((updated) => setSystemInfo(updated))
            .catch(() => {});
          setTimeout(async () => {
            await tauriApi.hideWindow().catch(() => {});
            setActiveTab('status');
          }, 1200);
          return {
            ...prev,
            progress: 100,
            bytesTransferred: '8.25 GB',
            speed: '0 MB/s',
            eta: 'Complete!',
            isActive: false,
          };
        }
        return {
          ...prev,
          progress: next,
          bytesTransferred: `${transferred} GB`,
          speed: (3.1 + Math.random() * 0.4).toFixed(2) + ' GB/s',
          eta: `${Math.max(0.1, (2.4 * (1 - next / 100))).toFixed(1)}s`,
        };
      });
    }, 150);
  };

  return (
    <div className="w-full h-full min-h-[290px] bg-white text-zinc-900 select-none flex flex-col justify-between p-4 sm:p-5 border border-zinc-200/80 shadow-xs text-[13px] font-sans antialiased overflow-hidden">
      {/* 1. Header Lockup */}
      <Header 
        currentVersion={currentVersion}
        hasUpdate={Boolean(updateManifest)}
        targetVersion={updateManifest?.version}
        isChecking={isCheckingUpdate}
        onOpenUpdate={() => {
          setUpdateReason('manual');
          setActiveTab('update');
        }}
        onCheckUpdate={() => checkForUpdates('manual')}
      />

      {/* 2. Middle Content Area with Separate UI for Updating */}
      {activeTab === 'status' ? (
        <div className="flex flex-col justify-center my-auto">
          <StatusGrid 
            systemInfo={systemInfo} 
            hasUpdate={Boolean(updateManifest)}
            targetVersion={updateManifest?.version}
            onOpenUpdate={() => {
              setUpdateReason('manual');
              setActiveTab('update');
            }}
          />
        </div>
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
          onStartUpdate={() => handleStartUpdate()}
          onCancel={() => setActiveTab('status')}
          onRetry={() => handleStartUpdate()}
        />
      )}
    </div>
  );
}
