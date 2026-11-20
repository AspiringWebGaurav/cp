import { useState, useEffect, useRef } from 'react';
import { flushSync } from 'react-dom';
import { listen } from '@tauri-apps/api/event';
import { Header } from './components/Header';
import { StatusGrid } from './components/StatusGrid';
import { SettingsUninstallView } from './components/SettingsUninstallView';
import { TransferView } from './components/TransferView';
import { ConflictView } from './components/ConflictView';
import { type UpdateUIState, type UpdateTriggerReason } from './components/UpdateView';
import { tauriApi } from './services/tauriApi';
import type { SystemInfo, ClipboardFileInfo, TransferState, ProgressUpdate, ReleaseManifest, UpdateDownloadProgress, ConflictAction, FileConflictInfo, TransferErrorEvent } from './types';

function isNewerVersion(remote: string, current: string): boolean {
  const parse = (v: string): number[] =>
    v.replace(/^v+/, '').split('.').map((s) => parseInt(s, 10) || 0);
  const r = parse(remote);
  const c = parse(current);
  const maxLen = Math.max(r.length, c.length);
  for (let i = 0; i < maxLen; i++) {
    const rv = r[i] || 0;
    const cv = c[i] || 0;
    if (rv > cv) return true;
    if (rv < cv) return false;
  }
  return false;
}

export default function App() {
  const [currentVersion, setCurrentVersion] = useState<string>(() => {
    if (typeof window !== 'undefined') {
      const cached = localStorage.getItem('cp_version');
      if (cached) return cached;
    }
    return '0.0.1';
  });

  const [systemInfo, setSystemInfo] = useState<SystemInfo>(() => {
    if (typeof window !== 'undefined') {
      try {
        const cached = localStorage.getItem('cp_system_info');
        if (cached) {
          const parsed = JSON.parse(cached);
          if (parsed && parsed.computerName) return parsed;
        }
      } catch (_) {}
    }
    const cachedVer = typeof window !== 'undefined' ? localStorage.getItem('cp_version') || '0.0.1' : '0.0.1';
    return {
      computerName: 'GAURAV',
      status: 'ON (intercepting)',
      hardware: 'SATA HDD + WDC NVMe SSD',
      engineVersion: cachedVer,
      transfersCompleted: 0,
      volumeMoved: '0 B',
      speedRating: 'PCIe 3.0 / 3.4 GB/s',
      processor: 'Intel Core i5-9300HF',
      memory: '16 GB RAM',
    };
  });

  const applyVersionUpdate = (ver?: string | null) => {
    if (!ver) return;
    const clean = ver.trim().replace(/^v/, '');
    setCurrentVersion(clean);
    setSystemInfo((prev) => ({ ...prev, engineVersion: clean }));
    if (typeof window !== 'undefined') {
      localStorage.setItem('cp_version', clean);
    }
  };

  const [clipboardFiles, setClipboardFiles] = useState<ClipboardFileInfo | null>(null);

  const [activeTasks, setActiveTasks] = useState<TransferState[]>(() => {
    if (typeof window !== 'undefined') {
      try {
        const cached = sessionStorage.getItem('cp_active_tasks');
        if (cached) return JSON.parse(cached);
      } catch (_) {}
    }
    return [];
  });

  const [focusedTaskId, setFocusedTaskId] = useState<string | null>(() => {
    if (typeof window !== 'undefined') {
      return sessionStorage.getItem('cp_focused_task_id');
    }
    return null;
  });

  const [transfer, setTransfer] = useState<TransferState>(() => {
    if (typeof window !== 'undefined') {
      try {
        const cached = sessionStorage.getItem('cp_active_transfer');
        if (cached) return JSON.parse(cached);
      } catch (_) {}
    }
    return {
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
    };
  });

  // Strict Dual-Entity Architecture: Window routing via query param (mode=landing vs mode=transfer)
  const searchParams = typeof window !== 'undefined' ? new URLSearchParams(window.location.search) : null;
  const urlMode = searchParams?.get('mode'); // 'landing' | 'transfer' | null
  const isTransferWindow = urlMode === 'transfer';
  const isLandingWindow = urlMode === 'landing';

  // Strict Dual-Entity Persistence: Landing Console and Copy/Paste UI are independent entities.
  // When user is on TransferView, hard refresh strictly preserves TransferView with all transfer state.
  // When user is on Landing Console, hard refresh strictly preserves Landing Console.
  const [activeTab, setActiveTab] = useState<'status' | 'transfer' | 'update' | 'conflict' | 'settings'>(() => {
    if (isTransferWindow) {
      return 'transfer';
    }
    if (isLandingWindow) {
      return 'status';
    }
    if (typeof window !== 'undefined') {
      const last = sessionStorage.getItem('cp_last_tab');
      if (last === 'status' || last === 'transfer' || last === 'update') {
        return last;
      }
    }
    return 'status';
  });

  // Conflict Resolution State
  const [conflicts, setConflicts] = useState<FileConflictInfo[]>([]);
  const [currentConflictIndex, setCurrentConflictIndex] = useState(0);
  const conflictDecisionsRef = useRef<Record<string, ConflictAction>>({});
  const pendingPasteTargetRef = useRef<ClipboardFileInfo | null>(null);

  // Update State Machine
  const [updateManifest, setUpdateManifest] = useState<ReleaseManifest | null>(null);
  const [updateState, setUpdateState] = useState<UpdateUIState>('ready');
  const [updateReason, setUpdateReason] = useState<UpdateTriggerReason>('startup');
  const [updateProgress, setUpdateProgress] = useState<UpdateDownloadProgress | null>(null);
  const [_updateError, setUpdateError] = useState<string | null>(null);
  const [isCheckingUpdate, setIsCheckingUpdate] = useState(false);

  // Synchronous State Refs to prevent stale closures in global event hooks
  const updateManifestRef = useRef<ReleaseManifest | null>(null);
  const updateStateRef = useRef<UpdateUIState>('ready');
  const updateReasonRef = useRef<UpdateTriggerReason>('startup');
  const isUpdatingInProgress = useRef(false);
  const userDismissedUpdate = useRef(false);
  const lastUpdateCheckTime = useRef(0);
  const isCheckingUpdateRef = useRef(false);
  const checkEpochRef = useRef(0);
  const isPastingInProgressRef = useRef(false);
  const lastPasteTimeRef = useRef(0);
  const activeTabRef = useRef<'status' | 'transfer' | 'update' | 'conflict' | 'settings'>(activeTab);
  const focusedTaskIdRef = useRef<string | null>(null);
  const hideTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const hasTransferErrorRef = useRef(false);
  const lastTransferRequestRef = useRef<{
    target: ClipboardFileInfo;
    conflictDecisions: Record<string, ConflictAction>;
    startIndex: number;
  } | null>(null);

  const clearHideTimeout = () => {
    if (hideTimeoutRef.current !== null) {
      clearTimeout(hideTimeoutRef.current);
      hideTimeoutRef.current = null;
    }
  };

  useEffect(() => {
    if (typeof window !== 'undefined') {
      sessionStorage.setItem('cp_last_tab', activeTab);
      if (activeTab === 'transfer' && transfer.source) {
        sessionStorage.setItem('cp_active_transfer', JSON.stringify(transfer));
        sessionStorage.setItem('cp_active_tasks', JSON.stringify(activeTasks));
        if (focusedTaskId) {
          sessionStorage.setItem('cp_focused_task_id', focusedTaskId);
        }
      } else if (activeTab === 'status') {
        sessionStorage.removeItem('cp_active_transfer');
        sessionStorage.removeItem('cp_active_tasks');
        sessionStorage.removeItem('cp_focused_task_id');
      }
    }
    updateManifestRef.current = updateManifest;
    updateStateRef.current = updateState;
    updateReasonRef.current = updateReason;
    activeTabRef.current = activeTab;
    focusedTaskIdRef.current = focusedTaskId;
    hasTransferErrorRef.current = Boolean(transfer.error);
  }, [updateManifest, updateState, updateReason, activeTab, focusedTaskId, transfer, activeTasks]);

  useEffect(() => {
    if (activeTab === 'transfer') {
      tauriApi.setUiMode('transfer').catch(() => {});
    }
  }, [activeTab]);

  const handleStartUpdate = async (targetManifest?: ReleaseManifest) => {
    let target = targetManifest || updateManifestRef.current;
    if (!target || isUpdatingInProgress.current) return;
    isUpdatingInProgress.current = true;
    const isPendingPaste = updateReasonRef.current === 'copy';

    // Fresh Pre-Flight Check: Ensure we download the absolute latest release published on GPHost
    // (e.g. if owner pushed 0.0.3 after 0.0.2 was presented, immediately upgrade target to 0.0.3)
    try {
      const liveCheck = await tauriApi.checkUpdate(currentVersion);
      if (liveCheck && liveCheck.version) {
        target = liveCheck;
        updateManifestRef.current = liveCheck;
        setUpdateManifest(liveCheck);
      }
    } catch (_) {}

    setUpdateState('downloading');
    setUpdateError(null);
    try {
      await tauriApi.downloadAndInstallUpdate(target, isPendingPaste, activeTabRef.current);
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
      const resolved = ver || info?.engineVersion;
      if (resolved) {
        applyVersionUpdate(resolved);
      }
      if (info) {
        setSystemInfo((prev) => ({
          ...info,
          engineVersion: resolved || info.engineVersion || prev.engineVersion,
        }));
        try {
          localStorage.setItem('cp_system_info', JSON.stringify(info));
        } catch (_) {}
      }
    } catch (e) {
      console.warn('Telemetry refresh warning:', e);
    }
  };

  const checkForUpdates = async (reason: UpdateTriggerReason = 'landing', _force = false) => {
    // 1. Concurrency Lock: If already checking right now, avoid overlapping requests
    if (isCheckingUpdateRef.current) {
      return;
    }

    const now = Date.now();
    // 2. Anti-spam debounce: prevent rapid accidental double-clicks within 500ms (unless force=true)
    if (!_force && now - lastUpdateCheckTime.current < 500) {
      return;
    }

    isCheckingUpdateRef.current = true;
    lastUpdateCheckTime.current = now;
    const epoch = ++checkEpochRef.current;

    // Only show spinning "Checking..." badge in Header when on the landing page or on manual click
    const showVisualFeedback = reason === 'landing' || reason === 'manual';
    if (showVisualFeedback) {
      setIsCheckingUpdate(true);
    }
    const checkStartTime = Date.now();

    try {
      const ver = await tauriApi.getAppVersion().catch(() => currentVersion);
      if (checkEpochRef.current !== epoch) return;
      if (ver) {
        applyVersionUpdate(ver);
      }

      // Max 8.0s timeout race so it can NEVER get stuck or hang indefinitely
      const checkPromise = tauriApi.checkUpdate(ver);
      const timeoutPromise = new Promise<null>((resolve) => setTimeout(() => resolve(null), 8000));
      const manifest = await Promise.race([checkPromise, timeoutPromise]).catch(() => null);

      if (checkEpochRef.current !== epoch) return;

      // Clean 380ms visual spin for smooth, polished visual confirmation
      if (showVisualFeedback) {
        const elapsed = Date.now() - checkStartTime;
        if (elapsed < 380) {
          await new Promise((r) => setTimeout(r, 380 - elapsed));
        }
      }

      if (checkEpochRef.current !== epoch) return;

      const cleanVer = (ver || currentVersion).replace(/^v+/, '');
      const remoteVer = manifest?.version?.replace(/^v+/, '');

      // Robust Guard: Update is ONLY triggered if remote version is strictly newer semver than current
      const hasNewer = Boolean(remoteVer && remoteVer !== cleanVer && isNewerVersion(remoteVer, cleanVer));

      if (manifest && manifest.version && hasNewer) {
        updateManifestRef.current = manifest;

        // If triggered by Landing UI or manual check (user is opening/viewing the UI)
        if (reason === 'landing' || reason === 'manual') {
          // CRITICAL GUARD: If ANY file transfer is actively in progress, DO NOT apply or restart!
          const isBusy = transfer.isActive || activeTasks.some((t) => t.isActive);
          const isEngineBusy = await tauriApi.isTransferRunning().catch(() => false);
          if (isBusy || isEngineBusy) {
            console.log('[CP Update] File transfer actively in progress. Update safely deferred.');
            setUpdateManifest(manifest);
            setUpdateReason(reason);
            setUpdateState('ready');
            return;
          }

          setUpdateManifest(manifest);
          setUpdateReason(reason);

          // Evaluate if staged binary is already ready on disk
          const staged = await tauriApi.checkStagedUpdate(manifest.version);
          if (staged) {
            setUpdateState('restarting');
            try {
              await tauriApi.applyInProcessUpdate(staged, manifest.version, 'status', false);
              return;
            } catch (e) {
              console.warn('[CP] Staged update apply error, fallback to full update:', e);
            }
          }

          // Not yet staged: initiate download & staging in UI
          setUpdateState('downloading');
          if (!isUpdatingInProgress.current) {
            handleStartUpdate(manifest);
          }
        } else {
          // In background / tray: quiet staging only, NEVER activate, install or restart!
          tauriApi.stageUpdateInBackground(manifest).catch(() => {});
        }
      } else {
        // Current version is already up to date (or remote is older/same): show current version and settle!
        updateManifestRef.current = null;
        updateStateRef.current = 'ready';
        setUpdateManifest(null);
        setUpdateState('ready');
      }
    } catch (err: any) {
      console.warn('Update check note:', err);
      if (checkEpochRef.current === epoch) {
        if (showVisualFeedback) {
          const elapsed = Date.now() - checkStartTime;
          if (elapsed < 380) {
            await new Promise((r) => setTimeout(r, 380 - elapsed));
          }
        }
        setUpdateManifest(null);
        setUpdateState('ready');
      }
    } finally {
      if (checkEpochRef.current === epoch) {
        isCheckingUpdateRef.current = false;
        if (showVisualFeedback) {
          setIsCheckingUpdate(false);
        }
      }
    }
  };

  useEffect(() => {
    // Notify native host that React has mounted and painted the DOM (eliminates white flash on launch)
    tauriApi.notifyAppReady().catch(() => {});

    // Initial hardware probe on mount
    refreshTelemetry();
    let mountCheckTimer: any = null;
    if (activeTabRef.current === 'status') {
      tauriApi.setUiMode('landing').catch(() => {});
      // Robust Landing Boot: Ensure update check triggers reliably whenever landing console mounts and is visible
      mountCheckTimer = setTimeout(async () => {
        const isVisible = await tauriApi.isMainWindowVisible().catch(() => false);
        if (isVisible) {
          checkForUpdates('landing', true);
        } else {
          checkForUpdates('startup', false);
        }
      }, 150);
    } else if (activeTabRef.current === 'transfer') {
      tauriApi.setUiMode('transfer').catch(() => {});
    }

    // Polling interval for live hardware & transfer telemetry (4 seconds, ONLY when console window is actively visible)
    // When hidden in system tray: zero polling, zero wakeups, near-zero CPU and battery consumption
    const interval = setInterval(async () => {
      const isVisible = await tauriApi.isMainWindowVisible().catch(() => false);
      if (isVisible && activeTabRef.current === 'status') {
        refreshTelemetry();
      }
    }, 4000);

    // Periodic update check: every 60s when console is visible on screen, quiet staging when quiet in tray
    const bgUpdateInterval = setInterval(async () => {
      const isVisible = await tauriApi.isMainWindowVisible().catch(() => false);
      if (isVisible && activeTabRef.current === 'status') {
        const now = Date.now();
        if (now - lastUpdateCheckTime.current > 45000) {
          checkForUpdates('landing', true);
        }
      } else {
        checkForUpdates('startup', false);
      }
    }, 60000);

    // Enterprise Automated Lifecycle & Clipboard State Restoration
    tauriApi.restoreLifecycleState()
      .then(async (lifecycle) => {
        if (lifecycle) {
          if (lifecycle.current_version) {
            applyVersionUpdate(lifecycle.current_version);
          }
          if (lifecycle.was_updated) {
            console.log(`[Lifecycle] Successfully updated from ${lifecycle.previous_version} to ${lifecycle.current_version}`);
            setUpdateManifest(null);
            setUpdateState('ready');

            // CRITICAL: Wipe stale pre-restart tasks so pre-restart and post-restart tasks are NEVER duplicated!
            sessionStorage.removeItem('cp_active_tasks');
            sessionStorage.removeItem('cp_active_transfer');
            sessionStorage.removeItem('cp_focused_task_id');
            setActiveTasks([]);

            if (lifecycle.has_pending_paste) {
              if (!isLandingWindow) {
                const clip = lifecycle.clipboard_files || (await tauriApi.getClipboardFiles().catch(() => null));
                if (clip && clip.filePaths && clip.filePaths.length > 0) {
                  setClipboardFiles(clip);
                  flushSync(() => {
                    setActiveTab('transfer');
                  });
                  handlePasteWithCP(clip, true);
                }
              }
              // If isLandingWindow, remain completely silent so transfer window executes cleanly
            } else if (!isTransferWindow && (lifecycle.restored_tab === 'status' || activeTabRef.current === 'status')) {
              setActiveTab('status');
              tauriApi.setUiMode('landing').catch(() => {});
            }

            if (lifecycle.is_base_reset || (lifecycle.was_updated && lifecycle.current_version === '0.0.1')) {
              // Owner reset to base executed: automatically probe and pull clean latest release!
              console.log('[Lifecycle] Reset to base complete. Probing latest release...');
              checkForUpdates('startup', true);
            }
          }
        }
      })
      .catch((e) => console.warn('Lifecycle restoration note:', e));

    let unlistenStats: (() => void) | undefined;
    listen('stats-reset', () => {
      refreshTelemetry();
    }).then((u) => { unlistenStats = u; }).catch(() => {});

    let unlistenAutoPaste: (() => void) | undefined;
    if (!isLandingWindow) {
      listen('trigger-auto-paste', async () => {
        try {
          clearHideTimeout();
          const now = Date.now();
          if (now - lastPasteTimeRef.current < 250) {
            return;
          }

          // Fast-path version sync on paste so TransferView Header is 100% accurate
          tauriApi.getAppVersion().then((v) => {
            if (v) applyVersionUpdate(v);
          }).catch(() => {});

          const clip = await tauriApi.getClipboardFiles();
          if (clip && clip.filePaths && clip.filePaths.length > 0) {
            setClipboardFiles(clip);
            handlePasteWithCP(clip);
          }
        } catch (e) {
          console.error('Auto paste failed:', e);
        }
      }).then((u) => { unlistenAutoPaste = u; }).catch(() => {});
    }

    // Event-Based: Whenever landing console window is opened from tray or explicit user trigger
    let unlistenShowConsole: (() => void) | undefined;
    if (!isTransferWindow) {
      listen('show-console', async () => {
        // Guard: Never interrupt an active in-flight transfer
        if (activeTabRef.current === 'transfer' && isPastingInProgressRef.current && transfer.isActive) {
          return;
        }
        flushSync(() => {
          setActiveTab('status');
        });
        tauriApi.setUiMode('landing').catch(() => {});
        // 1. Fresh telemetry probe
        refreshTelemetry();
        // 2. Each time user clicks CP tray icon and opens UI, run 1 fresh check
        checkForUpdates('landing', true);
      }).then((u) => { unlistenShowConsole = u; }).catch(() => {});
    }

    // Explicit User Action: Check for Updates triggered from System Tray Menu
    let unlistenTriggerCheck: (() => void) | undefined;
    if (!isTransferWindow) {
      listen('trigger-update-check', async () => {
        flushSync(() => {
          setActiveTab('status');
        });
        tauriApi.setUiMode('landing').catch(() => {});
        refreshTelemetry();
        checkForUpdates('manual', false);
      }).then((u) => { unlistenTriggerCheck = u; }).catch(() => {});
    }

    // Refresh local telemetry & check updates on window focus (throttled to 30s)
    const handleWindowFocus = () => {
      if (activeTabRef.current === 'status') {
        refreshTelemetry();
        const now = Date.now();
        if (now - lastUpdateCheckTime.current > 30000) {
          checkForUpdates('landing', false);
        }
      }
    };
    window.addEventListener('focus', handleWindowFocus);

    // Dynamic power, working set memory optimization & update verification on visibility changes
    const handleVisibilityChange = async () => {
      const isVisible = await tauriApi.isMainWindowVisible().catch(() => false);
      if (isVisible) {
        if (activeTabRef.current === 'status') {
          refreshTelemetry();
          // Regularly check for updates when landing page is un-hidden / revealed (throttled to 15s)
          const now = Date.now();
          if (now - lastUpdateCheckTime.current > 15000) {
            checkForUpdates('landing', true);
          }
        }
      } else {
        // Window hidden in tray: trim process memory footprint immediately
        tauriApi.trimMemory().catch(() => {});
      }
    };
    document.addEventListener('visibilitychange', handleVisibilityChange);

    let unlistenProgress: (() => void) | undefined;
    let unlistenTransferError: (() => void) | undefined;
    let unlistenFocusTask: (() => void) | undefined;
    let unlistenTaskCancelled: (() => void) | undefined;
    let unlistenTaskPaused: (() => void) | undefined;

    if (!isLandingWindow) {
      listen<ProgressUpdate>('transfer-progress', (event) => {
        const data = event.payload;
        const taskId = data.taskId;

        setActiveTasks((prev) => {
          const exists = prev.some((t) => t.taskId === taskId);
          if (exists) {
            return prev.map((t) => {
              if (!taskId || t.taskId === taskId) {
                return {
                  ...t,
                  progress: data.percentage,
                  bytesTransferred: data.formattedCopied,
                  totalBytes: data.formattedTotal,
                  speed: data.speed,
                  eta: data.isFinished ? 'Complete!' : data.eta,
                  isActive: !data.isFinished,
                  error: null,
                };
              }
              return t;
            });
          } else if (taskId) {
            // Register concurrent task dynamically
            return [
              ...prev,
              {
                taskId,
                isActive: !data.isFinished,
                isPaused: false,
                source: '',
                destination: '',
                currentFile: taskId,
                progress: data.percentage,
                speed: data.speed,
                bytesTransferred: data.formattedCopied,
                totalBytes: data.formattedTotal,
                eta: data.isFinished ? 'Complete!' : data.eta,
                error: null,
              },
            ];
          }
          return prev;
        });

        setTransfer((prev) => {
          const isCurrentFocus =
            !taskId ||
            !prev.taskId ||
            prev.taskId === taskId ||
            focusedTaskIdRef.current === taskId;

          if (isCurrentFocus) {
            return {
              ...prev,
              taskId: taskId || prev.taskId,
              progress: data.percentage,
              bytesTransferred: data.formattedCopied,
              totalBytes: data.formattedTotal,
              speed: data.speed,
              eta: data.isFinished ? 'Complete!' : data.eta,
              isActive: !data.isFinished,
              error: null,
            };
          }
          return prev;
        });
      }).then((u) => { unlistenProgress = u; }).catch(() => {});

      listen<TransferErrorEvent>('transfer-error', (event) => {
        const payload = event.payload;
        if (!payload?.error) return;
        const targetId = payload.taskId;
        clearHideTimeout();
        hasTransferErrorRef.current = true;
        setActiveTasks((prev) =>
          prev.map((t) =>
            !targetId || t.taskId === targetId
              ? { ...t, isActive: false, isPaused: false, speed: 'Failed', eta: 'Error', error: payload.error }
              : t
          )
        );
        setTransfer((prev) => {
          if (!targetId || !prev.taskId || prev.taskId === targetId || focusedTaskIdRef.current === targetId) {
            return {
              ...prev,
              isActive: false,
              isPaused: false,
              speed: 'Failed',
              eta: 'Error',
              error: payload.error,
            };
          }
          return prev;
        });
        tauriApi.showTransferWindow().catch(() => {});
      }).then((u) => { unlistenTransferError = u; }).catch(() => {});

      // Listen to focus-task event from task tray icon click
      listen<string>('focus-task', (event) => {
        const targetId = event.payload;
        flushSync(() => {
          setFocusedTaskId(targetId);
          focusedTaskIdRef.current = targetId;
          setActiveTab('transfer');
          setActiveTasks((prev) => {
            const found = prev.find((t) => t.taskId === targetId);
            if (found) {
              setTransfer(found);
            }
            return prev;
          });
        });
        tauriApi.showWindow().catch(() => {});
      }).then((u) => { unlistenFocusTask = u; }).catch(() => {});

      // Listen to task-cancelled event from task tray context menu or backend
      listen<string>('task-cancelled', (event) => {
        const targetId = event.payload;
        handleTaskCompletedOrCancelled(targetId, true);
      }).then((u) => { unlistenTaskCancelled = u; }).catch(() => {});

      // Listen to task-paused-changed event from task tray context menu
      listen<{ taskId: string; isPaused: boolean }>('task-paused-changed', (event) => {
        const { taskId, isPaused } = event.payload;
        setActiveTasks((prev) =>
          prev.map((t) => (t.taskId === taskId ? { ...t, isPaused } : t))
        );
        setTransfer((prev) =>
          prev.taskId === taskId ? { ...prev, isPaused } : prev
        );
      }).then((u) => { unlistenTaskPaused = u; }).catch(() => {});
    }

    // Listen to real-time update download progress events from Rust
    let unlistenUpdateProgress: (() => void) | undefined;
    listen<UpdateDownloadProgress>('update-download-progress', (event) => {
      const data = event.payload;
      // Only reflect download progress in UI if window is actively visible
      if (document.visibilityState === 'visible' && activeTabRef.current === 'status') {
        setUpdateProgress(data);
        if (data.stage === 'downloading') {
          setUpdateState('downloading');
        } else if (data.stage === 'verifying') {
          setUpdateState('verifying');
        } else if (data.stage === 'ready') {
          setUpdateState('ready');
        }
      }
    }).then((u) => { unlistenUpdateProgress = u; }).catch(() => {});

    // Listen to real-time server push release broadcast from Rust (SSE stream)
    let unlistenReleaseBroadcast: (() => void) | undefined;
    listen<ReleaseManifest>('ota-release-broadcast', async (event) => {
      const manifest = event.payload;
      if (manifest && manifest.version) {
        updateManifestRef.current = manifest;
        // In background: Rust already stages the update binary silently to disk.
        // DO NOT install, DO NOT restart, DO NOT show surprise UI while in tray.
        const isVisible = await tauriApi.isMainWindowVisible().catch(() => false);
        if (isVisible && activeTabRef.current === 'status') {
          setUpdateManifest(manifest);
          setUpdateReason('push_broadcast');
        }
      }
    }).then((u) => { unlistenReleaseBroadcast = u; }).catch(() => {});

    // Keyboard shortcuts & refresh policy:
    // In development (DEV): hard refresh and refresh remain fully accessible for developer velocity.
    // In production (PROD): hard refresh (Ctrl+F5, Ctrl+Shift+R) and refresh (F5, Ctrl+R) are strictly locked.
    const handleKeyDown = (e: KeyboardEvent) => {
      if (import.meta.env.PROD) {
        const isF5 = e.key === 'F5';
        const isCtrlR = (e.ctrlKey || e.metaKey) && (e.key === 'r' || e.key === 'R');
        if (isF5 || isCtrlR) {
          e.preventDefault();
          e.stopPropagation();
          return;
        }
      }

      if (e.key === 'Escape' && activeTabRef.current === 'transfer') {
        if (hasTransferErrorRef.current) {
          handleDismissError();
        } else {
          handleCancelTransfer();
        }
      }
    };

    const handleContextMenu = (e: MouseEvent) => {
      // In production, lock default browser context menu to prevent manual browser reload
      if (import.meta.env.PROD) {
        e.preventDefault();
      }
    };

    window.addEventListener('keydown', handleKeyDown, true);
    window.addEventListener('contextmenu', handleContextMenu);

    return () => {
      if (mountCheckTimer) clearTimeout(mountCheckTimer);
      clearInterval(interval);
      clearInterval(bgUpdateInterval);
      window.removeEventListener('focus', handleWindowFocus);
      document.removeEventListener('visibilitychange', handleVisibilityChange);
      window.removeEventListener('keydown', handleKeyDown, true);
      window.removeEventListener('contextmenu', handleContextMenu);
      if (unlistenStats) unlistenStats();
      if (unlistenAutoPaste) unlistenAutoPaste();
      if (unlistenShowConsole) unlistenShowConsole();
      if (unlistenTriggerCheck) unlistenTriggerCheck();
      if (unlistenProgress) unlistenProgress();
      if (unlistenTransferError) unlistenTransferError();
      if (unlistenFocusTask) unlistenFocusTask();
      if (unlistenTaskCancelled) unlistenTaskCancelled();
      if (unlistenTaskPaused) unlistenTaskPaused();
      if (unlistenUpdateProgress) unlistenUpdateProgress();
      if (unlistenReleaseBroadcast) unlistenReleaseBroadcast();
    };
  }, []);

  const handleTaskCompletedOrCancelled = (taskId: string, isCancel = false) => {
    setActiveTasks((prev) => {
      const remaining = prev.filter((t) => t.taskId !== taskId && t.isActive);
      if (remaining.length > 0) {
        if (focusedTaskIdRef.current === taskId || transfer.taskId === taskId) {
          const next = remaining[0];
          setFocusedTaskId(next.taskId || null);
          setTransfer(next);
        }
      } else {
        if (isCancel) {
          setTransfer((p) => ({ ...p, isActive: false, isPaused: false, progress: 0, error: null }));
        } else {
          setTransfer((p) => ({
            ...p,
            isActive: false,
            isPaused: false,
            progress: 100,
            eta: 'Complete!',
            error: null,
          }));
        }
        hasTransferErrorRef.current = false;
        setFocusedTaskId(null);
        if (typeof window !== 'undefined') {
          sessionStorage.removeItem('cp_active_transfer');
          sessionStorage.removeItem('cp_active_tasks');
          sessionStorage.removeItem('cp_focused_task_id');
          if (!isTransferWindow) {
            sessionStorage.setItem('cp_last_tab', 'status');
          }
        }
        if (isCancel) {
          clearHideTimeout();
          tauriApi.hideWindow().catch(() => {});
          tauriApi.trimMemory().catch(() => {});
          refreshTelemetry();
        } else {
          clearHideTimeout();
          hideTimeoutRef.current = setTimeout(async () => {
            await tauriApi.hideWindow().catch(() => {});
            await tauriApi.trimMemory().catch(() => {});
            setTransfer((p) => ({ ...p, progress: 0 }));
            refreshTelemetry();
            hideTimeoutRef.current = null;
          }, 800);
        }
      }
      return prev.filter((t) => t.taskId !== taskId);
    });
    setConflicts([]);
    pendingPasteTargetRef.current = null;
    conflictDecisionsRef.current = {};
  };

  const startActualTransfer = async (
    target: ClipboardFileInfo,
    conflictDecisions: Record<string, ConflictAction>,
    _skipUpdateCheck = false,
    startIndex = 0
  ) => {
    // Transactional State Reset: Ensure clean slate for transfer execution
    setConflicts([]);
    pendingPasteTargetRef.current = null;
    conflictDecisionsRef.current = {};
    hasTransferErrorRef.current = false;
    lastTransferRequestRef.current = { target, conflictDecisions, startIndex };
    if (typeof window !== 'undefined') {
      if (sessionStorage.getItem('cp_last_tab') === 'conflict') {
        sessionStorage.setItem('cp_last_tab', 'transfer');
      }
    }

    // Check if ALL files were skipped
    const allSkipped = target.filePaths.length > 0 && target.filePaths.every((p) => conflictDecisions[p] === 'skip');
    if (allSkipped) {
      isPastingInProgressRef.current = false;
      if (!isTransferWindow) {
        setActiveTab('status');
      }
      tauriApi.hideWindow().catch(() => {});
      return;
    }

    const destDir = target.activeExplorerPath || 'C:\\';
    const firstIdx = Math.min(startIndex, Math.max(0, target.filePaths.length - 1));
    const firstSrc = target.filePaths[firstIdx];
    const firstFileName = firstSrc.split(/[/\\]/).pop() || target.primaryName;
    const taskId = 'task-' + Date.now();

    const initialTransfer: TransferState = {
      taskId,
      isActive: true,
      isPaused: false,
      isCut: target.isCut,
      source: firstSrc,
      destination: destDir,
      currentFile: target.filePaths.length > 1 ? `(${firstIdx + 1}/${target.filePaths.length}) ${firstFileName}` : firstFileName,
      progress: 0,
      speed: 'Starting...',
      bytesTransferred: '0 B',
      totalBytes: target.formattedSize,
      eta: 'Calculating...',
      error: null,
    };

    // 1. Force synchronous DOM switch to 'transfer' BEFORE window is revealed
    flushSync(() => {
      setActiveTab('transfer');
      setTransfer(initialTransfer);
      setFocusedTaskId(taskId);
      setActiveTasks([initialTransfer]);
    });

    tauriApi.setUiMode('transfer').catch(() => {});

    // 2. Guarantee painted frame before atomically configuring and showing native window
    requestAnimationFrame(async () => {
      await tauriApi.showTransferWindow().catch(() => {});
    });

    try {
      for (let i = startIndex; i < target.filePaths.length; i++) {
        if (isUpdatingInProgress.current) {
          break;
        }

        const src = target.filePaths[i];
        const action = conflictDecisions[src] || 'keep_both';
        if (action === 'skip') {
          continue;
        }

        lastTransferRequestRef.current = { target, conflictDecisions, startIndex: i };
        const fileName = src.split(/[/\\]/).pop() || target.primaryName;

        setTransfer((prev) => ({
          ...prev,
          source: src,
          destination: destDir,
          isCut: target.isCut,
          currentFile: target.filePaths.length > 1 ? `(${i + 1}/${target.filePaths.length}) ${fileName}` : fileName,
          error: null,
        }));
        setActiveTasks((prev) =>
          prev.map((t) =>
            t.taskId === taskId
              ? {
                  ...t,
                  source: src,
                  destination: destDir,
                  currentFile: target.filePaths.length > 1 ? `(${i + 1}/${target.filePaths.length}) ${fileName}` : fileName,
                  error: null,
                }
              : t
          )
        );

        const updated = await tauriApi.executeCopyTransfer(src, destDir, target.isCut, taskId, action);
        setSystemInfo(updated);
        if (updated?.engineVersion) {
          applyVersionUpdate(updated.engineVersion);
        }
      }

      if (!isUpdatingInProgress.current) {
        lastTransferRequestRef.current = null;
        handleTaskCompletedOrCancelled(taskId);
      }
    } catch (err: any) {
      const errorMessage =
        typeof err === 'string'
          ? err
          : err?.message || String(err || 'Unexpected transfer error');
      const isCancelled = errorMessage.toLowerCase().includes('cancel');
      if (isCancelled) {
        if (!isUpdatingInProgress.current) {
          handleTaskCompletedOrCancelled(taskId, true);
        }
      } else if (!isUpdatingInProgress.current) {
        console.error('Transfer failed:', errorMessage);
        clearHideTimeout();
        hasTransferErrorRef.current = true;
        setActiveTasks((prev) =>
          prev.map((t) =>
            t.taskId === taskId
              ? { ...t, isActive: false, isPaused: false, speed: 'Failed', eta: 'Error', error: errorMessage }
              : t
          )
        );
        setTransfer((prev) => ({
          ...prev,
          isActive: false,
          isPaused: false,
          speed: 'Failed',
          eta: 'Error',
          error: errorMessage,
        }));
        tauriApi.showTransferWindow().catch(() => {});
      }
    } finally {
      isPastingInProgressRef.current = false;
    }
  };

  const handleRetryTransfer = () => {
    const req = lastTransferRequestRef.current;
    if (!req) return;
    hasTransferErrorRef.current = false;
    lastPasteTimeRef.current = 0;
    startActualTransfer(req.target, req.conflictDecisions, true, req.startIndex);
  };

  const handleDismissError = async () => {
    clearHideTimeout();
    hasTransferErrorRef.current = false;
    lastTransferRequestRef.current = null;
    lastPasteTimeRef.current = 0;
    setConflicts([]);
    pendingPasteTargetRef.current = null;
    conflictDecisionsRef.current = {};
    setActiveTasks([]);
    setFocusedTaskId(null);
    setTransfer((p) => ({ ...p, isActive: false, isPaused: false, progress: 0, error: null }));
    if (typeof window !== 'undefined') {
      sessionStorage.removeItem('cp_active_transfer');
      sessionStorage.removeItem('cp_active_tasks');
      sessionStorage.removeItem('cp_focused_task_id');
      if (!isTransferWindow) {
        sessionStorage.setItem('cp_last_tab', 'status');
      }
    }
    await tauriApi.hideWindow().catch(() => {});
    refreshTelemetry();
  };

  const handleResolveConflict = (action: ConflictAction, applyToAll: boolean) => {
    const current = conflicts[currentConflictIndex];
    if (!current) return;

    if (applyToAll) {
      const newDecisions = { ...conflictDecisionsRef.current };
      for (let i = currentConflictIndex; i < conflicts.length; i++) {
        newDecisions[conflicts[i].sourcePath] = action;
      }
      conflictDecisionsRef.current = newDecisions;
      if (pendingPasteTargetRef.current) {
        startActualTransfer(pendingPasteTargetRef.current, newDecisions);
      }
      return;
    }

    const updatedDecisions = {
      ...conflictDecisionsRef.current,
      [current.sourcePath]: action,
    };
    conflictDecisionsRef.current = updatedDecisions;

    if (currentConflictIndex + 1 < conflicts.length) {
      setCurrentConflictIndex((prev) => prev + 1);
    } else {
      if (pendingPasteTargetRef.current) {
        startActualTransfer(pendingPasteTargetRef.current, updatedDecisions);
      }
    }
  };

  const handlePasteWithCP = async (info?: ClipboardFileInfo, skipUpdateCheck = false) => {
    clearHideTimeout();
    const now = Date.now();
    if (now - lastPasteTimeRef.current < 250) {
      return;
    }
    lastPasteTimeRef.current = now;
    isPastingInProgressRef.current = true;

    const target = info || clipboardFiles;
    if (!target || !target.filePaths || target.filePaths.length === 0) {
      isPastingInProgressRef.current = false;
      return;
    }

    const destDir = target.activeExplorerPath || 'C:\\';

    // Pre-Paste Update Verification Pipeline:
    // When user copy/pastes and no transfer operations are actively running:
    // Check if an update is available or staged.
    // If yes: open UI holding state, save pending paste, apply update, and restart into updated version!
    if (!skipUpdateCheck && !isUpdatingInProgress.current) {
      const isBusy = transfer.isActive || activeTasks.some((t) => t.isActive);
      const isEngineBusy = await tauriApi.isTransferRunning().catch(() => false);

      if (!isBusy && !isEngineBusy) {
        try {
          const liveVer = await tauriApi.getAppVersion().catch(() => currentVersion);

          // 1. Check if an update binary is already staged on disk
          let staged = await tauriApi.checkAnyStagedUpdate().catch(() => null);
          let targetManifest: ReleaseManifest | null = null;

          // 2. If not staged yet, check GPHost with a 1.5-second timeout
          if (!staged) {
            const manifestPromise = tauriApi.checkUpdate(liveVer);
            const timeoutPromise = new Promise<null>((resolve) => setTimeout(() => resolve(null), 1500));
            targetManifest = await Promise.race([manifestPromise, timeoutPromise]);
          }

          const hasNewer = staged
            ? isNewerVersion(staged.version, liveVer)
            : Boolean(targetManifest?.version && isNewerVersion(targetManifest.version, liveVer));

          if (hasNewer) {
            const newerVer = staged?.version || targetManifest?.version || 'new';

            // OPEN UI IMMEDIATELY so user dynamically sees CP holding for update
            flushSync(() => {
              setActiveTab('transfer');
              setTransfer({
                taskId: 'update-restart',
                isActive: true,
                isPaused: false,
                isCut: target.isCut,
                source: target.filePaths[0] || '',
                destination: destDir,
                currentFile: target.primaryName,
                progress: 35,
                speed: 'HOLDING FOR UPDATE',
                bytesTransferred: '',
                totalBytes: target.formattedSize,
                eta: `Updating to v${newerVer}...`,
                error: null,
              });
            });
            tauriApi.setUiMode('transfer').catch(() => {});
            await tauriApi.showTransferWindow().catch(() => {});

            // If not yet staged, download and stage it now
            if (!staged && targetManifest) {
              const stagedPath = await Promise.race([
                tauriApi.stageUpdateInBackground(targetManifest),
                new Promise<null>((resolve) => setTimeout(() => resolve(null), 4500)),
              ]);
              if (stagedPath) {
                staged = { version: targetManifest.version, path: stagedPath };
              }
            }

            if (staged && staged.path && staged.version) {
              const finalStaged = staged;
              isUpdatingInProgress.current = true;

              // Comfortable visual pacing: advance progress bar and hold state
              setTransfer((p) => ({
                ...p,
                progress: 68,
                speed: 'HOLDING FOR UPDATE',
                eta: `Applying v${finalStaged.version}...`,
              }));
              await new Promise((r) => setTimeout(r, 700));

              // Save pending paste target explicitly in memory/disk
              await tauriApi.savePendingPaste(target);

              // Update card to indicate restart
              setTransfer((p) => ({
                ...p,
                speed: 'RESTARTING ENGINE',
                eta: `Launching v${finalStaged.version}...`,
                progress: 100,
              }));

              // Comfortable visual pause (850ms) for clean and readable transition
              await new Promise((r) => setTimeout(r, 850));

              // In-process swap & detached restart with --auto-paste
              await tauriApi.applyInProcessUpdate(finalStaged.path, finalStaged.version, 'transfer', true);
              isPastingInProgressRef.current = false;
              return;
            }
          }
        } catch (updateErr) {
          console.warn('[Pre-Paste Update Check] Proceeding with paste:', updateErr);
        }
      }
    }

    // 1. Conflict Detection: check if any of the target files already exist in destination
    try {
      const detected = await tauriApi.checkFileConflicts(target.filePaths, destDir, target.isCut);
      if (detected && detected.length > 0) {
        pendingPasteTargetRef.current = target;
        conflictDecisionsRef.current = {};
        setConflicts(detected);
        setCurrentConflictIndex(0);
        flushSync(() => {
          setActiveTab('conflict');
        });
        tauriApi.setUiMode('conflict').catch(() => {});
        await tauriApi.showTransferWindow().catch(() => {});
        isPastingInProgressRef.current = false;
        return;
      } else {
        setConflicts([]);
        pendingPasteTargetRef.current = null;
        conflictDecisionsRef.current = {};
      }
    } catch (e) {
      console.warn('Conflict detection note, proceeding cleanly:', e);
      setConflicts([]);
      pendingPasteTargetRef.current = null;
      conflictDecisionsRef.current = {};
    }

    // 2. Zero conflict: start direct high-speed transfer immediately
    startActualTransfer(target, {}, skipUpdateCheck);
  };

  const handleTogglePause = async () => {
    const nextState = !transfer.isPaused;
    setTransfer((p) => ({ ...p, isPaused: nextState }));
    if (transfer.taskId) {
      setActiveTasks((prev) =>
        prev.map((t) => (t.taskId === transfer.taskId ? { ...t, isPaused: nextState } : t))
      );
    }
    await tauriApi.pauseTransfer(nextState, transfer.taskId).catch(() => {});
  };

  const handleCancelTransfer = async () => {
    clearHideTimeout();
    lastPasteTimeRef.current = 0;
    hasTransferErrorRef.current = false;
    lastTransferRequestRef.current = null;
    setConflicts([]);
    pendingPasteTargetRef.current = null;
    conflictDecisionsRef.current = {};
    if (activeTabRef.current === 'conflict') {
      setActiveTab('transfer');
    }
    const cancelledId = transfer.taskId;
    await tauriApi.cancelTransfer(cancelledId).catch(() => {});
    if (cancelledId) {
      handleTaskCompletedOrCancelled(cancelledId, true);
    } else {
      setTransfer((p) => ({ ...p, isActive: false, isPaused: false, progress: 0, error: null }));
      if (typeof window !== 'undefined') {
        sessionStorage.removeItem('cp_active_transfer');
        sessionStorage.removeItem('cp_active_tasks');
        sessionStorage.removeItem('cp_focused_task_id');
        if (!isTransferWindow) {
          sessionStorage.setItem('cp_last_tab', 'status');
        }
      }
      tauriApi.hideWindow().catch(() => {});
      refreshTelemetry();
    }
  };

  const handleMinimizeTransfer = async () => {
    await tauriApi.minimizeWindow().catch(() => {});
  };

  return (
    <div className="w-full h-full bg-white text-zinc-900 select-none flex flex-col justify-between px-3.5 pt-3 pb-3 text-[12px] font-sans antialiased overflow-hidden">
      {/* 1. Header Lockup */}
      <Header 
        currentVersion={currentVersion}
        hasUpdate={Boolean(updateManifest && updateState !== 'ready')}
        targetVersion={updateManifest?.version}
        isChecking={isCheckingUpdate}
        isUpdating={isUpdatingInProgress.current || updateState === 'downloading' || updateState === 'verifying' || updateState === 'restarting'}
        updateStage={updateProgress?.stage || (updateState as string)}
        updateProgressPct={updateProgress?.percentage || 0}
        onCheckUpdate={() => {
          userDismissedUpdate.current = false;
          checkForUpdates('manual', true);
        }}
        isSettingsOpen={activeTab === 'settings'}
        onToggleSettings={() => {
          setActiveTab((prev) => (prev === 'settings' ? 'status' : 'settings'));
        }}
      />

      {/* 2. Middle Content Area (Clean, cohesive structure without awkward voids) */}
      <div className="flex-1 flex flex-col justify-between min-h-0">
        {isTransferWindow ? (
          activeTab === 'conflict' && conflicts.length > 0 ? (
            <ConflictView
              conflict={conflicts[currentConflictIndex] || conflicts[0]}
              currentIndex={currentConflictIndex}
              totalConflicts={conflicts.length}
              onResolve={handleResolveConflict}
              onCancel={handleCancelTransfer}
            />
          ) : (
            <TransferView 
              transfer={transfer} 
              onTogglePause={handleTogglePause} 
              onCancel={handleCancelTransfer} 
              onMinimize={handleMinimizeTransfer}
              onOpenDestination={(dest) => tauriApi.openDestinationFolder(dest)}
              onRetry={lastTransferRequestRef.current ? handleRetryTransfer : undefined}
              onDismissError={handleDismissError}
            />
          )
        ) : isLandingWindow ? (
          activeTab === 'settings' ? (
            <SettingsUninstallView
              onBack={() => setActiveTab('status')}
            />
          ) : (
            <StatusGrid 
              systemInfo={systemInfo} 
            />
          )
        ) : (
          /* Fallback for single-window / test environments */
          activeTab === 'settings' ? (
            <SettingsUninstallView
              onBack={() => setActiveTab('status')}
            />
          ) : activeTab === 'transfer' ? (
            <TransferView 
              transfer={transfer} 
              onTogglePause={handleTogglePause} 
              onCancel={handleCancelTransfer} 
              onMinimize={handleMinimizeTransfer}
              onOpenDestination={(dest) => tauriApi.openDestinationFolder(dest)}
              onRetry={lastTransferRequestRef.current ? handleRetryTransfer : undefined}
              onDismissError={handleDismissError}
            />
          ) : activeTab === 'conflict' && conflicts.length > 0 ? (
            <ConflictView
              conflict={conflicts[currentConflictIndex] || conflicts[0]}
              currentIndex={currentConflictIndex}
              totalConflicts={conflicts.length}
              onResolve={handleResolveConflict}
              onCancel={handleCancelTransfer}
            />
          ) : (
            <StatusGrid 
              systemInfo={systemInfo} 
            />
          )
        )}
      </div>
    </div>
  );
}
