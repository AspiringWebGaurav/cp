import { CheckCircle2, Zap } from 'lucide-react';
import type { SystemInfo } from '../types';

interface StatusGridProps {
  systemInfo: SystemInfo;
}

export function StatusGrid({ systemInfo }: StatusGridProps) {
  return (
    <div className="grid grid-cols-2 gap-0 items-stretch px-2 sm:px-4 py-2">
      {/* Left Column (50% exact width) */}
      <div className="space-y-2.5 pr-6 border-r border-zinc-200/80 flex flex-col justify-center">
        <div className="flex items-center justify-between whitespace-nowrap">
          <span className="font-bold text-zinc-900">Computer:</span>
          <span className="font-semibold text-zinc-800">{systemInfo.computerName}</span>
        </div>
        
        <div className="flex items-center justify-between whitespace-nowrap">
          <span className="font-bold text-zinc-900">Status:</span>
          <span className="font-bold text-emerald-600 flex items-center gap-1.5">
            <CheckCircle2 className="w-4 h-4 text-emerald-500 shrink-0" />
            {systemInfo.status}
          </span>
        </div>

        <div className="flex items-center justify-between whitespace-nowrap">
          <span className="font-bold text-zinc-900">Hardware:</span>
          <span className="font-medium text-zinc-700">{systemInfo.hardware}</span>
        </div>

        <div className="flex items-center justify-between whitespace-nowrap">
          <span className="font-bold text-zinc-900">Completed:</span>
          <span className="font-medium text-zinc-700">{systemInfo.transfersCompleted} jobs ({systemInfo.volumeMoved})</span>
        </div>
      </div>

      {/* Right Column (50% exact width) */}
      <div className="space-y-2.5 pl-6 flex flex-col justify-center">
        <div className="flex items-center justify-between whitespace-nowrap">
          <span className="font-bold text-zinc-900">ENGINE Version:</span>
          <span className="font-semibold text-zinc-800">{systemInfo.engineVersion}</span>
        </div>

        <div className="flex items-center justify-between whitespace-nowrap">
          <span className="font-bold text-zinc-900">Copyright (C):</span>
          <span className="font-medium text-zinc-700">Gaurav Patil</span>
        </div>

        <div className="flex items-center justify-between whitespace-nowrap">
          <span className="font-bold text-zinc-900">Licensed to:</span>
          <span className="font-medium text-zinc-700">MIT Open Source</span>
        </div>

        <div className="flex items-center justify-between whitespace-nowrap">
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

