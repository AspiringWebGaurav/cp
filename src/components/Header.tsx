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
    <div className="flex items-center justify-between pb-2 mb-2 border-b border-zinc-200/80 px-0.5">
      <div className="flex items-center gap-2.5">
        <img 
          src={cpIcon} 
          alt="CP Logo" 
          className="w-8 h-8 rounded-lg shadow-2xs shrink-0 object-contain"
        />
        <div className="flex flex-col justify-center select-none text-left">
          <span className="font-black text-[20px] tracking-[-0.03em] text-zinc-900 leading-none">
            CP
          </span>
          <span className="text-[9.5px] font-bold text-emerald-600 tracking-[0.14em] uppercase leading-none mt-1">
            Copy. Paste. Faster.
          </span>
        </div>
      </div>

      <div className="flex items-center gap-1.5">
        {hasUpdate ? (
          <button
            onClick={onOpenUpdate}
            className="flex items-center gap-1.5 px-2.5 py-0.5 rounded-full bg-emerald-50 text-emerald-700 hover:bg-emerald-100 border border-emerald-300 text-[10.5px] font-semibold transition-all cursor-pointer shadow-2xs animate-pulse"
          >
            <Sparkles className="w-3.5 h-3.5 text-emerald-600" />
            <span>Update v{targetVersion?.replace(/^v/, '')} Ready</span>
          </button>
        ) : isChecking ? (
          <div
            title="Checking GPHost Cloudflare R2 for latest release..."
            className="flex items-center gap-1.5 px-2 py-0.5 rounded-full bg-emerald-50/80 border border-emerald-200/90 text-emerald-700 text-[10.5px] font-mono shadow-2xs select-none"
          >
            <RefreshCw className="w-3 h-3 animate-spin text-emerald-600 shrink-0" />
            <span>Checking...</span>
          </div>
        ) : (
          <button
            onClick={onCheckUpdate}
            title="GPHost release verified • Click to re-check"
            className="flex items-center gap-1.5 px-2 py-0.5 rounded text-[10.5px] font-mono text-zinc-600 hover:text-zinc-900 hover:bg-zinc-100 border border-transparent hover:border-zinc-200 transition-all cursor-pointer"
          >
            <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 shrink-0 animate-pulse" />
            <span>v{currentVersion}</span>
            <span className="text-[9px] text-zinc-400 font-sans font-medium">(latest)</span>
          </button>
        )}
      </div>
    </div>
  );
}
