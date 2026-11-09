import { ArrowRight, Play, Pause, X, Zap, HardDrive, Clock, Minus, FolderOpen } from 'lucide-react';
import type { TransferState } from '../types';
import { MarqueeText } from './MarqueeText';

interface TransferViewProps {
  transfer: TransferState;
  onTogglePause: () => void;
  onCancel: () => void;
  onMinimize: () => void;
  onOpenDestination?: (path: string) => void;
}

export function TransferView({
  transfer,
  onTogglePause,
  onCancel,
  onMinimize,
  onOpenDestination,
}: TransferViewProps) {
  const isHolding =
    Boolean(transfer.speed?.toLowerCase().includes('holding')) ||
    Boolean(transfer.eta?.toLowerCase().includes('updating'));

  return (
    <div className="flex flex-col justify-between h-full font-sans select-none">
      {/* 1. Source & Destination Card — High-contrast, highlighted enterprise routing */}
      <div className="bg-gradient-to-b from-white to-zinc-50/95 border border-zinc-200/90 rounded-lg p-2 space-y-1.5 shadow-2xs">
        <div className="flex items-center gap-2 min-w-0">
          <span className="text-[9.5px] uppercase font-mono font-bold tracking-wider text-zinc-700 bg-zinc-100 border border-zinc-200/90 px-1.5 py-0.5 rounded shrink-0 shadow-2xs select-none">
            FROM
          </span>
          <div className="min-w-0 flex-1">
            <MarqueeText
              text={transfer.source}
              autoScroll={true}
              maxWidthClass="w-full max-w-full"
              className="text-zinc-900 font-mono text-[11px] font-medium leading-tight tracking-tight"
            />
          </div>
        </div>
        <div className="flex items-center gap-2 pt-1 border-t border-zinc-200/60 min-w-0">
          <span className="text-[9.5px] uppercase font-mono font-bold tracking-wider text-emerald-700 bg-emerald-50 border border-emerald-300/80 px-1.5 py-0.5 rounded shrink-0 flex items-center gap-1 shadow-2xs select-none">
            TO <ArrowRight className="w-2.5 h-2.5 text-emerald-600 shrink-0" />
          </span>
          <div className="min-w-0 flex-1">
            <MarqueeText
              text={transfer.destination}
              autoScroll={true}
              maxWidthClass="w-full max-w-full"
              className="text-emerald-950 font-mono text-[11px] font-bold leading-tight tracking-tight select-text"
              title={transfer.destination}
            />
          </div>
          <button
            type="button"
            onClick={() => onOpenDestination && onOpenDestination(transfer.destination)}
            title={`Open ${transfer.destination} in File Explorer`}
            className="shrink-0 px-2 py-0.5 rounded text-[10px] font-semibold text-emerald-800 bg-emerald-50/90 hover:bg-emerald-100 active:bg-emerald-200 border border-emerald-300/80 hover:border-emerald-400 flex items-center gap-1 transition-all cursor-pointer shadow-2xs group active:scale-[0.98]"
          >
            <FolderOpen className="w-3 h-3 text-emerald-600 group-hover:text-emerald-700 transition-colors" />
            <span>Open</span>
          </button>
        </div>
      </div>

      {/* 2. Active File & Status Row */}
      <div className="space-y-0.5 pt-1">
        <div className="flex items-center justify-between text-[10.5px]">
          <span
            className={`inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[9px] font-bold uppercase tracking-wider ${
              isHolding
                ? 'bg-amber-100 text-amber-900 border border-amber-300 shadow-2xs'
                : transfer.isPaused
                ? 'bg-amber-100 text-amber-900 border border-amber-300'
                : transfer.isCut
                ? 'bg-indigo-100 text-indigo-900 border border-indigo-300'
                : 'bg-emerald-100 text-emerald-900 border border-emerald-300'
            }`}
          >
            <span
              className={`w-1.5 h-1.5 rounded-full ${
                isHolding
                  ? 'bg-amber-500 animate-pulse'
                  : transfer.isPaused
                  ? 'bg-amber-500'
                  : transfer.isCut
                  ? 'bg-indigo-500 animate-pulse'
                  : 'bg-emerald-500 animate-pulse'
              }`}
            />
            {isHolding
              ? 'HOLDING FOR UPDATE'
              : transfer.isPaused
              ? 'Paused'
              : transfer.isCut
              ? 'Moving (Cut)'
              : 'Streaming Direct'}
          </span>
          <div className="font-mono text-[10.5px]">
            <span className="text-zinc-900 font-bold">{transfer.bytesTransferred}</span>
            <span className="text-zinc-400 font-medium px-1">/</span>
            <span className="text-zinc-500 font-semibold">{transfer.totalBytes}</span>
          </div>
        </div>
        <div className="w-full min-w-0 pt-0.5">
          <MarqueeText
            text={transfer.currentFile}
            autoScroll={true}
            maxWidthClass="w-full max-w-full"
            className="text-[11.5px] font-bold text-zinc-950 leading-tight"
          />
        </div>
      </div>

      {/* 3. High-Precision Animated Progress Bar */}
      <div className="space-y-1.5">
        <div className="w-full bg-zinc-100/90 rounded-lg h-5 border border-zinc-200/90 shadow-inner relative overflow-hidden p-[1.5px] transform-gpu">
          <div
            className={`h-full rounded-md transition-[width] duration-200 ease-out relative overflow-hidden transform-gpu will-change-[width] ${
              isHolding || transfer.isPaused
                ? 'bg-gradient-to-r from-amber-500 via-amber-400 to-yellow-400 shadow-[0_0_12px_rgba(245,158,11,0.35)]'
                : 'bg-gradient-to-r from-emerald-600 via-emerald-500 to-teal-400 shadow-[0_0_12px_rgba(16,185,129,0.35)]'
            }`}
            style={{ width: isHolding ? '100%' : `${Math.max(1, Math.min(100, transfer.progress))}%` }}
          >
            {/* Top Gloss Highlight */}
            <div className="absolute inset-x-0 top-0 h-[45%] bg-gradient-to-b from-white/30 to-transparent pointer-events-none" />
            {(!transfer.isPaused || isHolding) && (
              <div className="absolute inset-0 cp-shimmer-overlay pointer-events-none transform-gpu" />
            )}
          </div>
          <span className="absolute inset-0 flex items-center justify-center font-mono font-extrabold text-[10.5px] text-zinc-900 pointer-events-none select-none drop-shadow-[0_1px_1px_rgba(255,255,255,0.85)] tracking-tight">
            {isHolding
              ? 'HOLDING • UPDATING ENGINE BEFORE PASTE'
              : `${transfer.progress.toFixed(1)}% ${transfer.isPaused ? '(Paused)' : ''}`}
          </span>
        </div>

        {/* 4. Telemetry Metrics Strip (Hardware-Accelerated Speed & Throughput HUD) */}
        <div className="bg-gradient-to-r from-zinc-50 via-zinc-100/60 to-zinc-50 border border-zinc-200/80 rounded-lg px-2.5 py-1.5 flex items-center justify-between text-[10.5px] font-mono shadow-2xs">
          <span className="flex items-center gap-1.5 min-w-0">
            <span className={`p-0.5 rounded ${isHolding || transfer.isPaused ? 'bg-amber-100 text-amber-600' : 'bg-emerald-100 text-emerald-600 animate-pulse'}`}>
              <Zap className="w-3 h-3 shrink-0" />
            </span>
            <strong className="text-zinc-500 text-[10px] uppercase font-bold tracking-wider">Speed:</strong>
            <span className={`font-mono font-extrabold text-[11px] tracking-tight ${isHolding || transfer.isPaused ? 'text-amber-600' : 'text-emerald-600'}`}>
              {transfer.isPaused ? 'Paused' : transfer.speed}
            </span>
          </span>
          <span className="flex items-center gap-1.5 min-w-0 text-zinc-600">
            <HardDrive className="w-3 h-3 text-zinc-400 shrink-0" />
            <span className="text-zinc-800 font-bold">{transfer.bytesTransferred}</span>
            <span className="text-zinc-400 font-normal">/ {transfer.totalBytes}</span>
          </span>
          <span className="flex items-center gap-1.5 min-w-0">
            <Clock className="w-3 h-3 text-zinc-400 shrink-0" />
            <strong className="text-zinc-500 text-[10px] uppercase font-bold tracking-wider">ETA:</strong>
            <span className={`font-mono font-bold ${isHolding || transfer.isPaused ? 'text-amber-600' : 'text-zinc-900'}`}>
              {transfer.isPaused ? 'Paused' : transfer.eta}
            </span>
          </span>
        </div>
      </div>

      {/* 5. Cohesive Action Controls */}
      <div className="border-t border-zinc-200/80 pt-1.5 pb-0.5 flex items-center justify-end gap-1.5 shrink-0">
        <button
          onClick={onMinimize}
          title="Minimize transfer window to background system tray"
          className="px-2.5 py-1 bg-zinc-100 hover:bg-zinc-200 text-zinc-700 hover:text-zinc-900 border border-zinc-300 font-semibold rounded text-[10.5px] flex items-center gap-1 cursor-pointer transition-all shadow-2xs active:scale-[0.98]"
        >
          <Minus className="w-3 h-3" />
          Minimize
        </button>
        {!isHolding && (
          <button
            onClick={onTogglePause}
            className={`px-3 py-1 font-semibold rounded text-[10.5px] flex items-center gap-1 cursor-pointer transition-all border shadow-2xs active:scale-[0.98] ${
              transfer.isPaused
                ? 'bg-amber-50 hover:bg-amber-100 text-amber-900 border-amber-400 shadow-amber-100/50'
                : 'bg-zinc-100 hover:bg-zinc-200 text-zinc-800 border-zinc-300'
            }`}
          >
            {transfer.isPaused ? (
              <>
                <Play className="w-3 h-3 text-amber-600 fill-amber-600" />
                Resume
              </>
            ) : (
              <>
                <Pause className="w-3 h-3 text-amber-600 fill-amber-600" />
                Pause
              </>
            )}
          </button>
        )}
        <button
          onClick={onCancel}
          className="px-2.5 py-1 bg-zinc-100 hover:bg-rose-50 text-rose-700 hover:text-rose-800 border border-zinc-300 hover:border-rose-300 font-semibold rounded text-[10.5px] flex items-center gap-1 cursor-pointer transition-all shadow-2xs active:scale-[0.98]"
        >
          <X className="w-3 h-3" />
          Cancel
        </button>
      </div>
    </div>
  );
}
