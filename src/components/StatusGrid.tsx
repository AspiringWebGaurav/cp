import { CheckCircle2, Zap, ArrowUpCircle } from 'lucide-react';
import type { SystemInfo } from '../types';

interface StatusGridProps {
  systemInfo: SystemInfo;
  hasUpdate?: boolean;
  targetVersion?: string;
  onOpenUpdate?: () => void;
}

export function StatusGrid({ systemInfo, hasUpdate, targetVersion, onOpenUpdate }: StatusGridProps) {
  return (
    <div className="grid grid-cols-2 gap-0 items-stretch px-1">
      {/* Left Column (50% exact width) */}
      <div className="space-y-2 pr-4 border-r border-zinc-200/80 flex flex-col justify-between">
        <div className="flex items-center justify-between whitespace-nowrap text-[12px]">
          <span className="font-bold text-zinc-900">Computer:</span>
          <span className="font-semibold text-zinc-800">{systemInfo.computerName}</span>
        </div>
        
        <div className="flex items-center justify-between whitespace-nowrap text-[12px]">
          <span className="font-bold text-zinc-900">Status:</span>
          <span className="font-bold text-emerald-600 flex items-center gap-1.5">
            <CheckCircle2 className="w-3.5 h-3.5 text-emerald-500 shrink-0" />
            {systemInfo.status}
          </span>
        </div>

        <div className="flex items-center justify-between whitespace-nowrap text-[12px]">
          <span className="font-bold text-zinc-900">Hardware:</span>
          <span className="font-medium text-zinc-700 truncate max-w-[170px]" title={systemInfo.hardware}>
            {systemInfo.hardware}
          </span>
        </div>

        <div className="flex items-center justify-between whitespace-nowrap text-[12px]">
          <span className="font-bold text-zinc-900">Completed:</span>
          <span className="font-medium text-zinc-700">{systemInfo.transfersCompleted} jobs ({systemInfo.volumeMoved})</span>
        </div>
      </div>

      {/* Right Column (50% exact width) */}
      <div className="space-y-2 pl-4 flex flex-col justify-between">
        <div className="flex items-center justify-between whitespace-nowrap text-[12px]">
          <span className="font-bold text-zinc-900">ENGINE Version:</span>
          <div className="flex items-center gap-1.5">
            <span className="font-semibold text-zinc-800">{systemInfo.engineVersion}</span>
            {hasUpdate && (
              <button
                onClick={onOpenUpdate}
                title="Click to install update"
                className="flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-bold bg-emerald-50 text-emerald-700 border border-emerald-200 hover:bg-emerald-100 transition-colors cursor-pointer"
              >
                <ArrowUpCircle className="w-3 h-3 text-emerald-600" />
                <span>v{targetVersion?.replace(/^v/, '')}</span>
              </button>
            )}
          </div>
        </div>

        <div className="flex items-center justify-between whitespace-nowrap text-[12px]">
          <span className="font-bold text-zinc-900">Processor:</span>
          <span className="font-medium text-zinc-700 truncate max-w-[170px]" title={systemInfo.processor}>
            {systemInfo.processor || 'Intel Core Processor'}
          </span>
        </div>

        <div className="flex items-center justify-between whitespace-nowrap text-[12px]">
          <span className="font-bold text-zinc-900">Memory:</span>
          <span className="font-medium text-zinc-700">{systemInfo.memory || '8 GB RAM'}</span>
        </div>

        <div className="flex items-center justify-between whitespace-nowrap text-[12px]">
          <span className="font-bold text-zinc-900">Speed Rating:</span>
          <span className="font-bold text-emerald-600 flex items-center gap-1.5">
            <Zap className="w-3.5 h-3.5 text-amber-500 fill-amber-500 shrink-0" />
            {systemInfo.speedRating}
          </span>
        </div>
      </div>
    </div>
  );
}
