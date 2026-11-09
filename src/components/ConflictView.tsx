import { useState } from 'react';
import { RefreshCw, SkipForward, Copy, AlertTriangle, FileText } from 'lucide-react';
import type { FileConflictInfo, ConflictAction } from '../types';
import { MarqueeText } from './MarqueeText';

interface ConflictViewProps {
  conflict: FileConflictInfo;
  currentIndex: number;
  totalConflicts: number;
  onResolve: (action: ConflictAction, applyToAll: boolean) => void;
  onCancel: () => void;
}

export function ConflictView({
  conflict,
  currentIndex,
  totalConflicts,
  onResolve,
  onCancel,
}: ConflictViewProps) {
  const [applyToAll, setApplyToAll] = useState(false);

  const hasMultiple = totalConflicts > 1;

  // Defensive field resolution ensures 100% reliable rendering regardless of serialization casing
  const fileName = conflict.fileName || (conflict as any).file_name || 'File';
  const sourcePath = conflict.sourcePath || (conflict as any).source_path || '';
  const destinationPath = conflict.destinationPath || (conflict as any).destination_path || '';
  const sourceSizeFormatted = conflict.sourceSizeFormatted || (conflict as any).source_size_formatted || '0 B';
  const existingSizeFormatted = conflict.existingSizeFormatted || (conflict as any).existing_size_formatted || '0 B';
  const sourceModified = conflict.sourceModified || (conflict as any).source_modified || 'Unknown';
  const existingModified = conflict.existingModified || (conflict as any).existing_modified || 'Unknown';

  return (
    <div className="flex flex-col justify-between h-full font-sans select-none text-zinc-800">
      {/* 1. Header & Conflict Badge */}
      <div className="flex items-center justify-between pb-1 border-b border-zinc-200/80">
        <div className="flex items-center gap-2">
          <div className="w-5 h-5 rounded-full bg-amber-100 border border-amber-300 flex items-center justify-center shrink-0">
            <AlertTriangle className="w-3 h-3 text-amber-700" />
          </div>
          <div>
            <h2 className="text-[12.5px] font-bold text-zinc-950 tracking-tight leading-none">
              File Already Exists in Destination
            </h2>
            <p className="text-[10px] text-zinc-500 font-medium leading-tight mt-0.5">
              Choose how CP should handle this file conflict
            </p>
          </div>
        </div>

        <span className="inline-flex items-center px-2 py-0.5 rounded-full text-[9.5px] font-mono font-bold bg-amber-50 text-amber-800 border border-amber-200 shadow-2xs">
          CONFLICT {currentIndex + 1} OF {totalConflicts}
        </span>
      </div>

      {/* 2. Side-by-Side Comparison Card (Incoming vs Existing) */}
      <div className="grid grid-cols-2 gap-2 my-1">
        {/* Incoming File */}
        <div className="bg-gradient-to-b from-white to-emerald-50/40 border border-emerald-200/90 rounded-lg p-2 shadow-2xs flex flex-col justify-between">
          <div>
            <div className="flex items-center justify-between mb-1">
              <span className="text-[9px] uppercase font-mono font-bold tracking-wider text-emerald-800 bg-emerald-100/80 px-1.5 py-0.5 rounded border border-emerald-300/80">
                INCOMING (NEW)
              </span>
              <span className="text-[10px] font-mono font-bold text-emerald-900">
                {sourceSizeFormatted}
              </span>
            </div>
            <div className="flex items-center gap-1.5 mt-1">
              <FileText className="w-3.5 h-3.5 text-emerald-600 shrink-0" />
              <MarqueeText
                text={fileName}
                autoScroll={true}
                maxWidthClass="w-full max-w-[210px]"
                className="font-bold text-[11px] text-zinc-950 font-mono tracking-tight"
                title={fileName}
              />
            </div>
          </div>
          <div className="pt-1.5 mt-1.5 border-t border-emerald-100/80 text-[9.5px] font-mono text-zinc-500 space-y-0.5">
            <div className="truncate" title={sourcePath}>
              <span className="text-zinc-400">Src:</span> {sourcePath}
            </div>
            <div>
              <span className="text-zinc-400">Date:</span> {sourceModified}
            </div>
          </div>
        </div>

        {/* Existing Destination File */}
        <div className="bg-gradient-to-b from-white to-zinc-50/80 border border-zinc-200/90 rounded-lg p-2 shadow-2xs flex flex-col justify-between">
          <div>
            <div className="flex items-center justify-between mb-1">
              <span className="text-[9px] uppercase font-mono font-bold tracking-wider text-zinc-700 bg-zinc-100 px-1.5 py-0.5 rounded border border-zinc-200">
                EXISTING IN DESTINATION
              </span>
              <span className="text-[10px] font-mono font-bold text-zinc-800">
                {existingSizeFormatted}
              </span>
            </div>
            <div className="flex items-center gap-1.5 mt-1">
              <FileText className="w-3.5 h-3.5 text-zinc-500 shrink-0" />
              <MarqueeText
                text={fileName}
                autoScroll={true}
                maxWidthClass="w-full max-w-[210px]"
                className="font-bold text-[11px] text-zinc-950 font-mono tracking-tight"
                title={fileName}
              />
            </div>
          </div>
          <div className="pt-1.5 mt-1.5 border-t border-zinc-100 text-[9.5px] font-mono text-zinc-500 space-y-0.5">
            <div className="truncate" title={destinationPath}>
              <span className="text-zinc-400">Dst:</span> {destinationPath}
            </div>
            <div>
              <span className="text-zinc-400">Date:</span> {existingModified}
            </div>
          </div>
        </div>
      </div>

      {/* 3. Multi-file Checkbox & Action Buttons */}
      <div className="space-y-1.5 pt-1 border-t border-zinc-200/70">
        {hasMultiple && (
          <div className="flex items-center gap-2 px-1">
            <label className="flex items-center gap-2 cursor-pointer text-[10.5px] font-medium text-zinc-700 hover:text-zinc-950 transition-colors">
              <input
                type="checkbox"
                checked={applyToAll}
                onChange={(e) => setApplyToAll(e.target.checked)}
                className="rounded border-zinc-300 text-emerald-600 focus:ring-emerald-500 w-3.5 h-3.5 cursor-pointer"
              />
              <span>Do this for all remaining conflicts ({totalConflicts - currentIndex} files)</span>
            </label>
          </div>
        )}

        <div className="grid grid-cols-4 gap-1.5">
          {/* Replace Button */}
          <button
            type="button"
            onClick={() => onResolve('replace', applyToAll)}
            className="flex flex-col items-center justify-center p-2 rounded-lg bg-emerald-600 hover:bg-emerald-500 active:bg-emerald-700 text-white font-bold transition-all shadow-xs cursor-pointer group active:scale-[0.98]"
            title="Overwrite existing file with incoming version"
          >
            <div className="flex items-center gap-1 text-[11px]">
              <RefreshCw className="w-3.5 h-3.5 text-emerald-200 group-hover:rotate-180 transition-transform duration-500" />
              <span>Replace</span>
            </div>
            <span className="text-[8.5px] font-normal text-emerald-100 mt-0.5">
              Overwrite file
            </span>
          </button>

          {/* Skip Button */}
          <button
            type="button"
            onClick={() => onResolve('skip', applyToAll)}
            className="flex flex-col items-center justify-center p-2 rounded-lg bg-zinc-100 hover:bg-zinc-200 active:bg-zinc-300 text-zinc-800 font-bold border border-zinc-300/80 transition-all shadow-2xs cursor-pointer group active:scale-[0.98]"
            title="Leave existing file untouched and skip copying"
          >
            <div className="flex items-center gap-1 text-[11px]">
              <SkipForward className="w-3.5 h-3.5 text-zinc-600 group-hover:translate-x-0.5 transition-transform" />
              <span>Skip</span>
            </div>
            <span className="text-[8.5px] font-normal text-zinc-500 mt-0.5">
              Don't transfer
            </span>
          </button>

          {/* Keep Both Button */}
          <button
            type="button"
            onClick={() => onResolve('keep_both', applyToAll)}
            className="flex flex-col items-center justify-center p-2 rounded-lg bg-indigo-50 hover:bg-indigo-100 active:bg-indigo-200 text-indigo-900 font-bold border border-indigo-200/90 transition-all shadow-2xs cursor-pointer group active:scale-[0.98]"
            title="Rename incoming file with - Copy"
          >
            <div className="flex items-center gap-1 text-[11px]">
              <Copy className="w-3.5 h-3.5 text-indigo-600" />
              <span>Keep Both</span>
            </div>
            <span className="text-[8.5px] font-normal text-indigo-600/80 mt-0.5">
              Rename (- Copy)
            </span>
          </button>

          {/* Cancel Operation */}
          <button
            type="button"
            onClick={onCancel}
            className="flex flex-col items-center justify-center p-2 rounded-lg bg-white hover:bg-rose-50 active:bg-rose-100 text-zinc-600 hover:text-rose-700 font-semibold border border-zinc-200 hover:border-rose-200 transition-all cursor-pointer shadow-2xs active:scale-[0.98]"
            title="Abort entire copy/paste operation"
          >
            <span className="text-[11px]">Cancel</span>
            <span className="text-[8.5px] font-normal text-zinc-400 mt-0.5">
              Abort transfer
            </span>
          </button>
        </div>
      </div>
    </div>
  );
}
