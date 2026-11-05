import cpIcon from '../assets/cp-icon.png';
import { Sparkles, RefreshCw } from 'lucide-react';

interface HeaderProps {
  currentVersion?: string;
  hasUpdate?: boolean;
  targetVersion?: string;
  isChecking?: boolean;
  onOpenUpdate?: () => void;
  onCheckUpdate?: () => void;
}

export function Header({
  currentVersion = '0.0.1',
  hasUpdate = false,
  targetVersion,
  isChecking = false,
  onOpenUpdate,
  onCheckUpdate,
}: HeaderProps) {
  return (
    <div className="flex items-center justify-between pt-1 pb-3 px-1">
      <div className="flex items-center gap-3.5">
        <img 
          src={cpIcon} 
          alt="CP Logo" 
          className="w-11 h-11 rounded-[11px] shadow-xs shrink-0 object-contain"
        />
        <div className="flex flex-col justify-center select-none text-left">
          <span className="font-black text-[24px] tracking-[-0.03em] text-zinc-900 leading-none">
            CP
          </span>
          <span className="text-[11px] font-bold text-emerald-600 tracking-[0.16em] uppercase leading-none mt-1.5">
            Copy. Paste. Faster.
          </span>
        </div>
      </div>

      <div className="flex items-center gap-2">
        {hasUpdate ? (
          <button
            onClick={onOpenUpdate}
            className="flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-emerald-50 text-emerald-700 hover:bg-emerald-100 border border-emerald-200 text-[11px] font-semibold transition-all cursor-pointer shadow-2xs animate-pulse"
          >
            <Sparkles className="w-3.5 h-3.5 text-emerald-600" />
            <span>Update v{targetVersion?.replace(/^v/, '')} Ready</span>
          </button>
        ) : (
          <button
            onClick={onCheckUpdate}
            title="Click to check for updates"
            className="flex items-center gap-1 px-2 py-0.5 rounded text-[11px] font-mono text-zinc-500 hover:text-zinc-800 hover:bg-zinc-100 transition-colors cursor-pointer"
          >
            {isChecking ? (
              <RefreshCw className="w-3 h-3 animate-spin text-zinc-400" />
            ) : null}
            <span>v{currentVersion}</span>
          </button>
        )}
      </div>
    </div>
  );
}
