import { useState, useEffect, useRef } from 'react';
import { Settings } from 'lucide-react';
import cpIcon from '../assets/cp-icon.png';

interface HeaderProps {
  currentVersion?: string;
  hasUpdate?: boolean;
  targetVersion?: string;
  isChecking?: boolean;
  isUpdating?: boolean;
  updateStage?: string;
  updateProgressPct?: number;
  onCheckUpdate?: () => void;
  isSettingsOpen?: boolean;
  onToggleSettings?: () => void;
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
  isSettingsOpen = false,
  onToggleSettings,
}: HeaderProps) {
  // Manual user click tracking to show "UP TO DATE" momentarily only on explicit user request (ScheduleMe parity)
  const wasManualClickRef = useRef(false);
  const [showResultStage, setShowResultStage] = useState(false);
  const resultTimerRef = useRef<number | null>(null);

  const handleButtonClick = () => {
    wasManualClickRef.current = true;
    onCheckUpdate?.();
  };

  const prevCheckingRef = useRef(isChecking);
  useEffect(() => {
    if (prevCheckingRef.current && !isChecking && !isUpdating && !hasUpdate) {
      if (wasManualClickRef.current) {
        setShowResultStage(true);
        if (resultTimerRef.current !== null) {
          window.clearTimeout(resultTimerRef.current);
        }
        resultTimerRef.current = window.setTimeout(() => {
          setShowResultStage(false);
          wasManualClickRef.current = false;
          resultTimerRef.current = null;
        }, 1800);
      }
    }
    prevCheckingRef.current = isChecking;
  }, [isChecking, isUpdating, hasUpdate]);

  const isVerifying = updateStage === 'verifying';
  const isInstalling =
    updateStage === 'staging' || updateStage === 'ready' || updateStage === 'restarting';
  const pct = Math.min(100, Math.max(8, Math.round(updateProgressPct || 0)));

  // ScheduleMe Stage Text Machine (Pure Blended, NO (latest), NO Green Dot)
  const otaStageText = (() => {
    if (isChecking) {
      return 'CHECKING';
    }
    if (showResultStage) {
      return 'UP TO DATE';
    }
    return '';
  })();

  const otaStageTone = (() => {
    if (isChecking) {
      return 'text-zinc-600 font-semibold animate-pulse';
    }
    return 'text-zinc-700 font-semibold';
  })();

  const stagePhaseKey = otaStageText ? (isChecking ? 'checking' : 'up_to_date') : '';

  const [displayedStage, setDisplayedStage] = useState(otaStageText);
  const [displayedTone, setDisplayedTone] = useState(otaStageTone);
  const [displayedPhaseKey, setDisplayedPhaseKey] = useState(stagePhaseKey);
  const [stageVisible, setStageVisible] = useState(Boolean(otaStageText));
  const hideTimerRef = useRef<number | null>(null);

  useEffect(() => {
    if (hideTimerRef.current !== null) {
      window.clearTimeout(hideTimerRef.current);
      hideTimerRef.current = null;
    }

    if (otaStageText) {
      setDisplayedStage(otaStageText);
      setDisplayedTone(otaStageTone);
      setDisplayedPhaseKey(stagePhaseKey);
      let raf2 = 0;
      const raf1 = window.requestAnimationFrame(() => {
        raf2 = window.requestAnimationFrame(() => {
          setStageVisible(true);
        });
      });
      return () => {
        window.cancelAnimationFrame(raf1);
        window.cancelAnimationFrame(raf2);
      };
    }

    setStageVisible(false);
    hideTimerRef.current = window.setTimeout(() => {
      setDisplayedStage('');
      setDisplayedPhaseKey('');
      hideTimerRef.current = null;
    }, 520);

    return () => {
      if (hideTimerRef.current !== null) {
        window.clearTimeout(hideTimerRef.current);
        hideTimerRef.current = null;
      }
    };
  }, [otaStageText, otaStageTone, stagePhaseKey]);

  const rowWashClass = !otaStageText
    ? 'opacity-0'
    : isChecking
    ? 'opacity-100 bg-zinc-200/40'
    : 'opacity-100 bg-zinc-100/60';

  const showShimmer = isChecking && stageVisible;

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

      {/* Right Lockup: Settings Gear & ScheduleMe Borderless Blended Dynamic Version UI */}
      <div className="relative flex items-center gap-1.5">
        {/* Settings Gear Icon Button with 90-degree Rotation Animation */}
        <button
          type="button"
          data-testid="header-settings-btn"
          onClick={onToggleSettings}
          title={isSettingsOpen ? 'Back to Dashboard' : 'Settings & Maintenance'}
          aria-label="Settings"
          className={`group relative p-1 rounded-md transition-colors duration-200 cursor-pointer focus:outline-none ${
            isSettingsOpen ? 'bg-zinc-100 text-zinc-900' : 'text-zinc-400 hover:text-zinc-700 hover:bg-zinc-100/80'
          }`}
        >
          <Settings
            className={`w-3.5 h-3.5 transform-gpu transition-transform duration-300 ease-out ${
              isSettingsOpen ? 'rotate-90 text-zinc-900' : 'rotate-0 text-zinc-400 group-hover:text-zinc-700'
            }`}
          />
        </button>

        {/* Version UI Lockup */}
        <div className="flex items-center" data-testid="header-version-lockup">
        {isUpdating || hasUpdate ? (
          <div
            title={`Automated background update to v${targetVersion?.replace(/^v/, '') || ''} • ${Math.round(updateProgressPct || 0)}%`}
            className="group relative overflow-hidden inline-flex items-center gap-1.5 px-1.5 py-1 rounded-md text-zinc-600 text-[11px] font-mono font-medium leading-none select-none transition-colors duration-200"
          >
            {/* Hairline Bottom Progress Bar (Neutral Zinc Track & Fill, Zero Green) */}
            <div
              aria-hidden="true"
              className="absolute inset-x-1 bottom-0 h-[1.5px] pointer-events-none overflow-hidden rounded-full bg-zinc-200"
            >
              <div
                className="h-full bg-zinc-600 transition-[width] duration-300 ease-out"
                style={{ width: `${isInstalling || isVerifying ? 100 : pct}%` }}
              />
            </div>

            {/* Symmetrically Aligned Single-Line Lockup (Pure Monospace Symmetry, Zero Spinner, Zero Ellipsis) */}
            <span className="relative z-10 inline-flex items-center leading-none whitespace-nowrap">
              <span className="font-semibold text-zinc-700 tracking-wide uppercase">
                {updateStage === 'restarting'
                  ? 'RESTARTING'
                  : updateStage === 'verifying'
                  ? `VERIFYING V${targetVersion?.replace(/^v/i, '') || ''}`
                  : isInstalling
                  ? `INSTALLING V${targetVersion?.replace(/^v/i, '') || ''}`
                  : targetVersion
                  ? `UPDATING TO V${targetVersion.replace(/^v/i, '')}${updateProgressPct > 0 ? ` ${Math.round(updateProgressPct)}%` : ''}`
                  : 'UPDATING'}
              </span>
            </span>
          </div>
        ) : (
          <button
            data-testid="header-version-btn"
            type="button"
            onClick={handleButtonClick}
            disabled={isChecking}
            title={
              isChecking
                ? 'Checking GPHost Cloudflare R2 for latest release...'
                : 'GPHost release verified • Click to re-check'
            }
            className="group relative overflow-hidden inline-flex items-center px-1.5 py-1 rounded-md text-zinc-500 hover:text-zinc-900 hover:bg-zinc-100/70 transition-colors duration-200 cursor-pointer select-none font-mono text-[11px] font-medium leading-none"
          >
            {/* Ambient Dynamic Background Wash */}
            <div
              aria-hidden="true"
              className={`absolute inset-0 pointer-events-none rounded-md transition-opacity duration-500 ease-out ${rowWashClass}`}
            />

            {/* Shimmer Sweep Effect (Active during checking) */}
            {showShimmer && (
              <div
                aria-hidden="true"
                className="absolute inset-0 pointer-events-none overflow-hidden rounded-md"
              >
                <div className="h-full w-2/3 animate-shimmer-sweep bg-gradient-to-r from-transparent via-zinc-900/[0.08] to-transparent" />
              </div>
            )}

            {/* Symmetrically Aligned Single-Line Lockup (Pure Single Baseline, Zero Bottom Inclination) */}
            <span className="relative z-10 inline-flex items-center leading-none whitespace-nowrap gap-1.5">
              <span className="tracking-wide uppercase">
                V{currentVersion?.replace(/^v/i, '')}
              </span>

              {/* Dynamic Stage Text (ScheduleMe in-place slide, fade, and expand on exact same line) */}
              {displayedStage ? (
                <span
                  data-testid="header-version-stage"
                  className={`inline-flex items-center leading-none overflow-hidden whitespace-nowrap text-[11px] tracking-wide transition-[opacity,max-width,color] duration-500 ease-out ${displayedTone} ${
                    stageVisible
                      ? 'opacity-100 max-w-[120px]'
                      : 'opacity-0 max-w-0'
                  }`}
                >
                  <span key={displayedPhaseKey} className="inline-block animate-fade-in font-semibold">
                    {displayedStage}
                  </span>
                </span>
              ) : null}
            </span>
          </button>
        )}
        </div>
      </div>
    </div>
  );
}
