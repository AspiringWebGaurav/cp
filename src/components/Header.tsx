import cpIcon from '../assets/cp-icon.png';
import { RefreshCw } from 'lucide-react';

interface HeaderProps {
  currentVersion?: string;
  hasUpdate?: boolean;
  targetVersion?: string;
  isChecking?: boolean;
  isUpdating?: boolean;
  updateStage?: string;
  updateProgressPct?: number;
  onCheckUpdate?: () => void;
}

export function Header({
  currentVersion = '0.0.1',
  hasUpdate = false,
  targetVersion,
  isChecking = false,
  isUpdating = false,
  updateStage = 'downloading',
  updateProgressPct = 0,
  onCheckUpdate,
}: HeaderProps) {
  return (
    <div className="flex items-center justify-between pb-2 mb-1.5 border-b border-zinc-200/75 px-0.5 select-none shrink-0">
      {/* Left Lockup: Brand & Identity */}
      <a
        href="https://cp.apps.gauravpatil.site"
        target="_blank"
        rel="noreferrer"
        title="CP by Gaurav Patil • cp.apps.gauravpatil.site"
        className="flex items-center gap-2.5 cursor-pointer group"
      >
        <img 
          src={cpIcon} 
          alt="CP Logo" 
          className="w-8 h-8 rounded-lg shadow-2xs ring-1 ring-zinc-900/5 shrink-0 object-contain transform-gpu transition-transform group-hover:scale-105 duration-200"
        />
        <div className="flex flex-col justify-center select-none text-left">
          <span className="font-extrabold text-[19px] tracking-tight text-zinc-900 leading-none group-hover:text-emerald-700 transition-colors">
            CP
          </span>
          <span className="text-[9px] font-bold text-emerald-600 tracking-[0.16em] uppercase leading-none mt-1">
            Copy. Paste. Faster.
          </span>
        </div>
      </a>

      {/* Right Lockup: Symmetrical Status Capsule */}
      <div className="flex items-center">
        {isUpdating || hasUpdate ? (
          <div
            title={`Automated background update to v${targetVersion?.replace(/^v/, '') || ''} • ${Math.round(updateProgressPct || 0)}%`}
            className="relative inline-flex items-center justify-center gap-1.5 px-3 py-1 rounded-full bg-emerald-50/90 border border-emerald-300/90 text-emerald-900 text-[11px] font-mono shadow-2xs select-none overflow-hidden min-w-[134px] transform-gpu transition-all"
          >
            {/* Integrated Fill Progress Bar */}
            <div
              className="absolute inset-y-0 left-0 bg-gradient-to-r from-emerald-200/80 via-emerald-300/85 to-emerald-400/90 transition-[width] duration-300 ease-out pointer-events-none"
              style={{
                width: `${
                  updateStage === 'restarting' || updateStage === 'staging' || updateStage === 'ready'
                    ? 100
                    : updateStage === 'verifying'
                    ? 100
                    : Math.min(100, Math.max(8, updateProgressPct || 0))
                }%`,
              }}
            />

            {/* Foreground Content (always crisp, stable, and jitter-free) */}
            <RefreshCw className="w-3.5 h-3.5 animate-spin text-emerald-700 shrink-0 transform-gpu relative z-10" />
            <span className="font-semibold text-emerald-950 relative z-10 tracking-tight whitespace-nowrap">
              {updateStage === 'restarting'
                ? 'Restarting...'
                : updateStage === 'verifying'
                ? 'Verifying...'
                : updateStage === 'staging' || updateStage === 'ready'
                ? 'Installing...'
                : targetVersion
                ? `Updating v${targetVersion.replace(/^v/, '')}${updateProgressPct > 0 ? ` ${Math.round(updateProgressPct)}%` : '...'}`
                : 'Updating...'}
            </span>
          </div>
        ) : isChecking ? (
          <div
            title="Checking GPHost Cloudflare R2 for latest release..."
            className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-emerald-50/70 border border-emerald-200/90 text-emerald-800 text-[11px] font-mono shadow-2xs select-none transform-gpu"
          >
            <RefreshCw className="w-3.5 h-3.5 animate-spin text-emerald-600 shrink-0 transform-gpu" />
            <span className="font-medium">Checking...</span>
          </div>
        ) : (
          <button
            onClick={onCheckUpdate}
            title="GPHost release verified • Click to re-check"
            className="group inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-zinc-50 hover:bg-zinc-100/90 border border-zinc-200/90 hover:border-zinc-300 text-zinc-700 hover:text-zinc-900 transition-all duration-200 cursor-pointer shadow-2xs transform-gpu hover:shadow-xs active:scale-[0.98]"
          >
            <span className="relative flex h-2 w-2 shrink-0">
              <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75 transform-gpu" />
              <span className="relative inline-flex rounded-full h-2 w-2 bg-emerald-500" />
            </span>
            <span className="text-[11px] font-mono font-semibold text-zinc-800">
              v{currentVersion}
            </span>
            <span className="text-[9.5px] font-semibold text-emerald-700 bg-emerald-50 px-1.5 py-0.2 rounded-full border border-emerald-200/70 tracking-wide">
              (latest)
            </span>
          </button>
        )}
      </div>
    </div>
  );
}
