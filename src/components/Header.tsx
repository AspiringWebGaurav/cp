import cpIcon from '../assets/cp-icon.png';

export function Header() {
  return (
    <div className="flex items-center justify-center pt-1 pb-3">
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
    </div>
  );
}

