import { useState, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { Header } from './components/Header';
import { StatusGrid } from './components/StatusGrid';
import { TransferView } from './components/TransferView';
import { tauriApi } from './services/tauriApi';
import type { SystemInfo, ClipboardFileInfo, TransferState, ProgressUpdate } from './types';

export default function App() {
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

  const [activeTab, setActiveTab] = useState<'status' | 'transfer'>('status');

  useEffect(() => {
    const refreshTelemetry = () => {
      tauriApi.getSystemInfo()
        .then((info) => setSystemInfo(info))
        .catch(() => {});
    };

    refreshTelemetry();
    const interval = setInterval(refreshTelemetry, 2000);

    let unlisten: (() => void) | undefined;
    listen('stats-reset', () => {
      refreshTelemetry();
    }).then((u) => { unlisten = u; }).catch(() => {});

    let unlistenAutoPaste: (() => void) | undefined;
    listen('trigger-auto-paste', async () => {
      try {
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
      setActiveTab('status');
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

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key.toLowerCase() === 'v') {
        handlePasteWithCP();
      } else if (e.key.toLowerCase() === 't') {
        startBenchmarkTransfer();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => {
      clearInterval(interval);
      if (unlisten) unlisten();
      if (unlistenAutoPaste) unlistenAutoPaste();
      if (unlistenShowConsole) unlistenShowConsole();
      if (unlistenProgress) unlistenProgress();
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [clipboardFiles]);

  const handlePasteWithCP = async (info?: ClipboardFileInfo) => {
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
      // Show brief completion then auto-hide to system tray!
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
      source: 'C:\\Downex\\Mockingjay_Part1_1080p.mkv',
      destination: 'D:\\',
      currentFile: 'Mockingjay_Part1_1080p.mkv',
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
      <Header />

      {/* 2. Middle Content Area */}
      {activeTab === 'status' ? (
        <div className="flex flex-col justify-center my-auto">
          <StatusGrid systemInfo={systemInfo} />
        </div>
      ) : (
        <TransferView 
          transfer={transfer} 
          onTogglePause={handleTogglePause} 
          onCancel={handleCancelTransfer} 
        />
      )}
    </div>
  );
}
