import { ArrowRight, Play, Pause, X, Zap, HardDrive, Clock } from 'lucide-react';
import type { TransferState } from '../types';

interface TransferViewProps {
  transfer: TransferState;
  onTogglePause: () => void;
  onCancel: () => void;
}

export function TransferView({ transfer, onTogglePause, onCancel }: TransferViewProps) {
  return (
    <div className="flex flex-col justify-between h-full space-y-1.5 font-sans">
      {/* 1. Source & Destination Card — Full paths, NO truncation (...) */}
      <div className="bg-zinc-50/90 border border-zinc-200/80 rounded-md p-1.5 text-[10.5px] space-y-1 shadow-2xs">
        <div className="flex items-start gap-1.5">
          <span className="text-[9.5px] uppercase font-bold tracking-wider text-zinc-500 shrink-0 w-10 pt-0.5">
            FROM
          </span>
          <span className="text-zinc-800 font-medium break-all leading-tight select-text">
            {transfer.source}
          </span>
        </div>
        <div className="flex items-start gap-1.5 pt-1 border-t border-zinc-200/60">
          <span className="text-[9.5px] uppercase font-bold tracking-wider text-emerald-600 shrink-0 w-10 pt-0.5 flex items-center gap-0.5">
            TO <ArrowRight className="w-2.5 h-2.5 text-emerald-500" />
          </span>
          <span className="text-zinc-900 font-semibold break-all leading-tight select-text">
            {transfer.destination}
          </span>
        </div>
      </div>

      {/* 2. Active File Info — Full filename, NO truncation (...) */}
      <div className="space-y-0.5">
        <div className="flex items-center justify-between text-[10.5px]">
          <span
            className={`inline-flex items-center gap-1 px-1.5 py-0.5 rounded-full text-[9px] font-bold uppercase tracking-wider ${
              transfer.isPaused
                ? 'bg-amber-100 text-amber-800 border border-amber-300/80'
                : 'bg-emerald-100 text-emerald-800 border border-emerald-300/80'
            }`}
          >
            <span
              className={`w-1.5 h-1.5 rounded-full ${
                transfer.isPaused ? 'bg-amber-500' : 'bg-emerald-500 animate-pulse'
              }`}
            />
            {transfer.isPaused ? 'Paused' : 'Streaming Direct'}
          </span>
          <span className="font-mono text-zinc-500 font-semibold">
            {transfer.bytesTransferred} / {transfer.totalBytes}
          </span>
        </div>
        <p className="text-[11px] font-semibold text-zinc-900 leading-tight truncate select-text">
          {transfer.currentFile}
        </p>
      </div>

      {/* 3. Ultra-Polished High-Precision Animated Progress Bar */}
      <div className="space-y-1">
        <div className="w-full bg-zinc-100/90 rounded-md h-4 border border-zinc-200/90 shadow-inner relative overflow-hidden p-0.5">
          <div
            className={`h-full rounded transition-all duration-200 ease-out relative overflow-hidden shadow-xs ${
              transfer.isPaused
                ? 'bg-gradient-to-r from-amber-500 to-amber-400'
                : 'bg-gradient-to-r from-emerald-600 via-emerald-500 to-teal-400'
            }`}
            style={{ width: `${Math.max(1, Math.min(100, transfer.progress))}%` }}
          >
            {!transfer.isPaused && (
              <div className="absolute inset-0 cp-shimmer-overlay pointer-events-none" />
            )}
          </div>
          <span className="absolute inset-0 flex items-center justify-center font-mono font-bold text-[9.5px] text-zinc-900 pointer-events-none select-none drop-shadow-[0_1px_1px_rgba(255,255,255,0.7)]">
            {transfer.progress.toFixed(1)}% {transfer.isPaused && '(Paused)'}
          </span>
        </div>

        {/* 4. Telemetry Metrics Row */}
        <div className="flex items-center justify-between text-[10px] font-mono text-zinc-700 px-0.5">
          <span className="flex items-center gap-1">
            <Zap className="w-3 h-3 text-amber-500 shrink-0" />
            <strong>Speed:</strong>{' '}
            <span className={transfer.isPaused ? 'text-amber-600 font-bold' : 'text-emerald-600 font-bold'}>
              {transfer.isPaused ? 'Paused' : transfer.speed}
            </span>
          </span>
          <span className="flex items-center gap-1">
            <HardDrive className="w-3 h-3 text-zinc-400 shrink-0" />
            <span>{transfer.bytesTransferred}</span>
          </span>
          <span className="flex items-center gap-1">
            <Clock className="w-3 h-3 text-zinc-400 shrink-0" />
            <strong>ETA:</strong>{' '}
            <span className={transfer.isPaused ? 'text-amber-600 font-semibold' : 'text-zinc-900 font-semibold'}>
              {transfer.isPaused ? 'Paused' : transfer.eta}
            </span>
          </span>
        </div>
      </div>

      {/* 5. Action Controls */}
      <div className="border-t border-zinc-200/80 pt-1.5 flex items-center justify-end gap-1.5">
        <button
          onClick={onTogglePause}
          className={`px-2.5 py-1 font-semibold rounded text-[10.5px] flex items-center gap-1 cursor-pointer transition-all border shadow-2xs ${
            transfer.isPaused
              ? 'bg-amber-50 hover:bg-amber-100 text-amber-900 border-amber-300'
              : 'bg-zinc-100 hover:bg-zinc-200 text-zinc-800 border-zinc-300'
          }`}
        >
          {transfer.isPaused ? (
            <>
              <Play className="w-3 h-3 text-emerald-600 fill-emerald-600" />
              Resume
            </>
          ) : (
            <>
              <Pause className="w-3 h-3 text-amber-600 fill-amber-600" />
              Pause
            </>
          )}
        </button>
        <button
          onClick={onCancel}
          className="px-2.5 py-1 bg-zinc-100 hover:bg-rose-50 text-rose-700 hover:text-rose-800 border border-zinc-300 hover:border-rose-300 font-semibold rounded text-[10.5px] flex items-center gap-1 cursor-pointer transition-all shadow-2xs"
        >
          <X className="w-3 h-3" />
          Cancel
        </button>
      </div>
    </div>
  );
}
