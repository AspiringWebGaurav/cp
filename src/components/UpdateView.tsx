import React from 'react';
import { Download, RefreshCw, AlertCircle, Loader2 } from 'lucide-react';
import type { ReleaseManifest, UpdateDownloadProgress } from '../types';

export type UpdateUIState = 'checking' | 'available' | 'downloading' | 'verifying' | 'ready' | 'restarting' | 'error';
export type UpdateTriggerReason = 'startup' | 'landing' | 'copy' | 'manual' | 'push_broadcast';

interface UpdateViewProps {
  currentVersion: string;
  manifest: ReleaseManifest | null;
  state: UpdateUIState;
  reason?: UpdateTriggerReason;
  progress: UpdateDownloadProgress | null;
  errorMessage: string | null;
  onStartUpdate: () => void;
  onCancel: () => void;
  onRetry: () => void;
}

export const UpdateView: React.FC<UpdateViewProps> = ({
  currentVersion,
  manifest,
  state,
  reason = 'startup',
  progress,
  errorMessage,
  onStartUpdate,
  onCancel,
  onRetry,
}) => {
  const targetVersion = manifest?.version ? `v${manifest.version.replace(/^v/, '')}` : 'latest';
  const percent = progress?.percentage || (state === 'verifying' || state === 'ready' || state === 'restarting' ? 100 : 0);

  const formatMb = (bytes: number) => {
    return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
  };

  const isUpdatingActive = state === 'downloading' || state === 'verifying' || state === 'ready' || state === 'restarting';

  return (
    <div className="flex flex-col justify-between h-full px-1 py-0.5 select-none animate-in fade-in duration-150">
      {/* 1. Header Row */}
      <div className="flex items-center justify-between border-b border-zinc-200/80 pb-1.5">
        <div className="flex items-center gap-2">
          {state === 'checking' && (
            <Loader2 className="w-3.5 h-3.5 text-zinc-500 animate-spin shrink-0" />
          )}
          {state === 'available' && (
            <Download className="w-3.5 h-3.5 text-emerald-600 shrink-0" />
          )}
          {state === 'downloading' && (
            <Download className="w-3.5 h-3.5 text-emerald-600 animate-bounce shrink-0" />
          )}
          {(state === 'verifying' || state === 'ready' || state === 'restarting') && (
            <RefreshCw className="w-3.5 h-3.5 text-emerald-600 animate-spin shrink-0" />
          )}
          {state === 'error' && (
            <AlertCircle className="w-3.5 h-3.5 text-rose-500 shrink-0" />
          )}

          <div className="flex items-center gap-1.5 font-bold text-[12px] text-zinc-900">
            {state === 'checking' && !manifest && <span>Checking for updates...</span>}
            {(state === 'available' || (state === 'checking' && manifest)) && <span>Update Available</span>}
            {isUpdatingActive && <span>Updating CP</span>}
            {state === 'error' && <span className="text-rose-600">Update Paused</span>}

            <span className="text-[10px] font-semibold font-mono px-1.5 py-0.2 rounded bg-emerald-50 text-emerald-700 border border-emerald-200">
              {targetVersion}
            </span>
          </div>
        </div>

        <div className="text-[10.5px] font-medium text-zinc-500">
          Current: <span className="font-mono text-zinc-700 font-semibold">v{currentVersion}</span>
        </div>
      </div>

      {/* 2. Middle Explanation & Progress */}
      <div className="py-1 space-y-1.5">
        {/* Friendly explanation banner */}
        <div className="bg-zinc-50 border border-zinc-200/80 rounded-md p-2 text-left">
          <p className="text-[11px] font-medium text-zinc-700 leading-snug">
            {state === 'checking' && !manifest ? (
              <>
                <strong className="text-zinc-900 font-semibold">Checking for updates.</strong> Connecting to server to check for the latest version...
              </>
            ) : reason === 'copy' ? (
              <>
                <strong className="text-zinc-900 font-semibold">A new update is available.</strong> Please hold on for a moment while CP updates itself before copying.
              </>
            ) : (
              <>
                <strong className="text-zinc-900 font-semibold">A new update is available.</strong> Please hold on while CP updates itself.
              </>
            )}
          </p>
        </div>

        {/* Progress Bar & Simple Stats */}
        {isUpdatingActive && (
          <div className="space-y-2">
            <div className="flex items-center justify-between text-[12px]">
              <span className="font-semibold text-zinc-800">
                {state === 'downloading' && 'Downloading update...'}
                {state === 'verifying' && '⚡ Verifying with Hardware Acceleration (AES-NI)...'}
                {(state === 'ready' || state === 'restarting') && 'Finishing up, restarting now...'}
              </span>
              <span className="font-mono font-bold text-zinc-900">{percent.toFixed(0)}%</span>
            </div>

            {/* Progress Bar Track */}
            <div className="w-full bg-zinc-100 rounded-full h-2.5 overflow-hidden border border-zinc-200">
              <div
                className="bg-emerald-500 h-full rounded-full transition-all duration-150 ease-out"
                style={{ width: `${percent}%` }}
              />
            </div>

            {/* Simple Metrics */}
            <div className="flex items-center justify-between text-[11px] text-zinc-500 font-mono">
              <span>
                {progress?.downloaded_bytes ? formatMb(progress.downloaded_bytes) : '0.0 MB'} /{' '}
                {progress?.total_bytes ? formatMb(progress.total_bytes) : (manifest?.size ? formatMb(manifest.size) : '18.4 MB')}
              </span>
              <span>
                {state === 'downloading'
                  ? (progress?.speed || 'Connecting...')
                  : state === 'verifying'
                  ? '⚡ AES-NI Hardware Decryption'
                  : 'Ready'}
              </span>
            </div>
          </div>
        )}

        {/* Error Notice */}
        {state === 'error' && (
          <div className="bg-rose-50 border border-rose-200/80 rounded-lg p-3 text-left">
            <div className="text-[12px] font-bold text-rose-800 mb-1">
              We couldn't finish the update
            </div>
            <p className="text-[11px] text-rose-700 leading-relaxed">
              {errorMessage || 'Please check your internet connection and try again.'}
            </p>
          </div>
        )}
      </div>

      {/* 3. Footer */}
      <div className="flex items-center justify-between pt-1.5 border-t border-zinc-200/80">
        <div className="text-[10.5px] text-zinc-500">
          {isUpdatingActive ? (
            <span>CP will restart automatically when finished.</span>
          ) : state === 'error' ? (
            <span>You can retry now or continue using CP.</span>
          ) : state === 'checking' && !manifest ? (
            <span>Connecting to update server...</span>
          ) : (
            <span>Update {targetVersion} ready to install.</span>
          )}
        </div>

        <div className="flex items-center gap-1.5">
          {(state === 'available' || (state === 'checking' && manifest)) && (
            <button
              onClick={onStartUpdate}
              className="px-3 py-1 text-[11px] font-semibold text-white bg-emerald-600 hover:bg-emerald-700 rounded-md transition-all shadow-xs flex items-center gap-1 cursor-pointer"
            >
              <Download className="w-3 h-3" />
              <span>Update Now</span>
            </button>
          )}

          {isUpdatingActive && (
            <div className="text-[10.5px] font-medium text-emerald-600 flex items-center gap-1.5 py-0.5">
              <RefreshCw className="w-3 h-3 animate-spin text-emerald-500" />
              <span>Please wait...</span>
            </div>
          )}

          {state === 'error' && (
            <>
              <button
                onClick={onCancel}
                className="px-2.5 py-1 text-[11px] font-medium text-zinc-600 hover:text-zinc-900 hover:bg-zinc-100 rounded-md transition-colors cursor-pointer"
              >
                Continue to CP
              </button>
              <button
                onClick={onRetry}
                className="px-3 py-1 text-[11px] font-semibold text-white bg-zinc-900 hover:bg-zinc-800 rounded-md transition-all flex items-center gap-1 cursor-pointer"
              >
                <RefreshCw className="w-3 h-3" />
                <span>Try Again</span>
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
};
