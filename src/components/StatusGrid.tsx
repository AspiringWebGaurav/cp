import { CheckCircle2, Zap } from 'lucide-react';
import type { SystemInfo } from '../types';
import { MarqueeText } from './MarqueeText';

interface StatusGridProps {
  systemInfo: SystemInfo;
}

export function StatusGrid({ systemInfo }: StatusGridProps) {
  return (
    <div className="bg-zinc-50/70 border border-zinc-200/80 rounded-xl px-4 py-3.5 grid grid-cols-2 gap-0 items-center select-none shadow-2xs">
      {/* Left Column (50% exact width) */}
      <div className="space-y-2.5 pr-4 border-r border-zinc-200/80 flex flex-col justify-center">
        <div className="flex items-center text-[12px] gap-2 whitespace-nowrap">
          <span className="font-bold text-zinc-900 w-[82px] shrink-0">Computer:</span>
          <MarqueeText
            text={systemInfo.computerName}
            maxWidthClass="max-w-[170px]"
            className="font-semibold text-zinc-800"
          />
        </div>
        
        <div className="flex items-center text-[12px] gap-2 whitespace-nowrap">
          <span className="font-bold text-zinc-900 w-[82px] shrink-0">Status:</span>
          <span className={`font-bold flex items-center gap-1.5 min-w-0 ${systemInfo.status.includes('PAUSED') ? 'text-amber-600' : 'text-emerald-600'}`}>
            <CheckCircle2 className={`w-3.5 h-3.5 shrink-0 ${systemInfo.status.includes('PAUSED') ? 'text-amber-500' : 'text-emerald-500'}`} />
            <MarqueeText
              text={systemInfo.status}
              maxWidthClass="max-w-[150px]"
              className={`font-bold ${systemInfo.status.includes('PAUSED') ? 'text-amber-600' : 'text-emerald-600'}`}
            />
          </span>
        </div>

        <div className="flex items-center text-[12px] gap-2 whitespace-nowrap">
          <span className="font-bold text-zinc-900 w-[82px] shrink-0">Hardware:</span>
          <MarqueeText
            text={systemInfo.hardware}
            maxWidthClass="max-w-[170px]"
            className="font-medium text-zinc-700"
          />
        </div>

        <div className="flex items-center text-[12px] gap-2 whitespace-nowrap">
          <span className="font-bold text-zinc-900 w-[82px] shrink-0">Completed:</span>
          <MarqueeText
            text={`${systemInfo.transfersCompleted} jobs (${systemInfo.volumeMoved})`}
            maxWidthClass="max-w-[170px]"
            className="font-medium text-zinc-700"
          />
        </div>
      </div>

      {/* Right Column (50% exact width) */}
      <div className="space-y-2.5 pl-4 flex flex-col justify-center">
        <div className="flex items-center text-[12px] gap-2 whitespace-nowrap">
          <span className="font-bold text-zinc-900 w-[106px] shrink-0">ENGINE Version:</span>
          <span className="font-semibold text-zinc-800">{systemInfo.engineVersion}</span>
        </div>

        <div className="flex items-center text-[12px] gap-2 whitespace-nowrap">
          <span className="font-bold text-zinc-900 w-[106px] shrink-0">Processor:</span>
          <MarqueeText
            text={systemInfo.processor || 'Intel Core Processor'}
            maxWidthClass="max-w-[155px]"
            className="font-medium text-zinc-700"
          />
        </div>

        <div className="flex items-center text-[12px] gap-2 whitespace-nowrap">
          <span className="font-bold text-zinc-900 w-[106px] shrink-0">Memory:</span>
          <MarqueeText
            text={systemInfo.memory || '8 GB RAM'}
            maxWidthClass="max-w-[155px]"
            className="font-medium text-zinc-700"
          />
        </div>

        <div className="flex items-center text-[12px] gap-2 whitespace-nowrap">
          <span className="font-bold text-zinc-900 w-[106px] shrink-0">Speed Rating:</span>
          <span className="font-bold text-emerald-600 flex items-center gap-1.5 min-w-0">
            <Zap className="w-3.5 h-3.5 text-amber-500 fill-amber-500 shrink-0" />
            <MarqueeText
              text={systemInfo.speedRating}
              maxWidthClass="max-w-[140px]"
              className="font-bold text-emerald-600"
            />
          </span>
        </div>
      </div>
    </div>
  );
}
