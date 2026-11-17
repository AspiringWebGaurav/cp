import { useState } from 'react';
import { ArrowLeft, Trash2, AlertTriangle } from 'lucide-react';
import { tauriApi } from '../services/tauriApi';

interface SettingsUninstallViewProps {
  onBack: () => void;
}

export function SettingsUninstallView({ onBack }: SettingsUninstallViewProps) {
  const [isConfirming, setIsConfirming] = useState(false);
  const [isLaunching, setIsLaunching] = useState(false);

  const handleLaunchUninstall = async () => {
    setIsLaunching(true);
    try {
      await tauriApi.launchUninstaller();
    } catch (err: unknown) {
      console.error('Failed to launch uninstaller:', err);
      setIsLaunching(false);
    }
  };

  return (
    <div className="relative w-full h-full flex flex-col justify-between">
      <div className="relative bg-zinc-50/70 border border-zinc-200/80 rounded-xl px-5 py-4 select-none shadow-2xs flex-1 flex flex-col justify-between overflow-hidden">
        {/* Top Navigation */}
        <div className="flex items-center justify-between pb-2 border-b border-zinc-200/60">
          <span className="text-[12px] font-semibold text-zinc-700">Settings</span>
          <button
            type="button"
            data-testid="settings-back-btn"
            onClick={onBack}
            className="flex items-center gap-1 text-[11px] font-medium text-zinc-500 hover:text-zinc-900 px-2 py-0.5 rounded-md hover:bg-zinc-200/60 transition-colors cursor-pointer"
          >
            <ArrowLeft className="w-3.5 h-3.5" />
            <span>Back</span>
          </button>
        </div>

        {/* Main Content: Simple Clean Uninstall */}
        {!isConfirming ? (
          <div className="bg-white border border-zinc-200/90 rounded-lg p-3.5 flex items-center justify-between my-auto shadow-2xs">
            <div className="flex items-center gap-3">
              <div className="p-2 rounded-lg bg-rose-50 text-rose-600 border border-rose-100 shrink-0">
                <Trash2 className="w-4 h-4" />
              </div>
              <div className="flex flex-col text-left">
                <span className="text-[12.5px] font-bold text-zinc-900 leading-tight">
                  Uninstall CP
                </span>
                <span className="text-[11px] text-zinc-500 mt-0.5">
                  Remove CP from your computer.
                </span>
              </div>
            </div>

            <button
              type="button"
              data-testid="open-uninstall-modal-btn"
              onClick={() => setIsConfirming(true)}
              className="px-3.5 py-1.5 text-xs font-semibold text-white bg-rose-600 hover:bg-rose-700 rounded-md transition-colors cursor-pointer shadow-xs"
            >
              Uninstall
            </button>
          </div>
        ) : (
          /* Confirmation State: Simple & Straightforward */
          <div
            data-testid="uninstall-confirm-modal"
            className="bg-white border border-rose-200 rounded-lg p-3.5 flex items-center justify-between my-auto shadow-2xs animate-in fade-in duration-150"
          >
            <div className="flex items-center gap-2.5">
              <div className="p-1.5 rounded-full bg-rose-100 text-rose-600 shrink-0">
                <AlertTriangle className="w-4 h-4" />
              </div>
              <div className="flex flex-col text-left">
                <span className="text-[12px] font-bold text-zinc-900">
                  Are you sure you want to uninstall CP?
                </span>
                <span className="text-[11px] text-zinc-500">
                  This will launch the Windows uninstaller.
                </span>
              </div>
            </div>

            <div className="flex items-center gap-2 shrink-0">
              <button
                type="button"
                data-testid="cancel-uninstall-btn"
                onClick={() => setIsConfirming(false)}
                disabled={isLaunching}
                className="px-3 py-1.5 text-xs font-medium text-zinc-700 bg-zinc-100 hover:bg-zinc-200 rounded-md transition-colors cursor-pointer"
              >
                Cancel
              </button>
              <button
                type="button"
                data-testid="confirm-launch-uninstaller-btn"
                onClick={handleLaunchUninstall}
                disabled={isLaunching}
                className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-semibold text-white bg-rose-600 hover:bg-rose-700 rounded-md transition-colors cursor-pointer disabled:opacity-60 shadow-xs"
              >
                <Trash2 className="w-3.5 h-3.5" />
                <span>{isLaunching ? 'Starting...' : 'Uninstall'}</span>
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
