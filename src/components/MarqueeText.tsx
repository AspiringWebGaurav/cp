import { useState, useRef, useEffect, useId } from 'react';

interface MarqueeTextProps {
  text?: string;
  className?: string;
  maxWidthClass?: string;
  title?: string;
  autoScroll?: boolean;
  onClick?: () => void;
}

export function MarqueeText({
  text = '',
  className = '',
  maxWidthClass = 'max-w-[175px]',
  title,
  autoScroll = true,
  onClick,
}: MarqueeTextProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const measureRef = useRef<HTMLSpanElement>(null);
  const [isHovered, setIsHovered] = useState(false);
  const [overflowDistance, setOverflowDistance] = useState(0);
  const [scrubOffset, setScrubOffset] = useState<number | null>(null);
  const uniqueId = useId();

  useEffect(() => {
    const measure = () => {
      if (containerRef.current && measureRef.current) {
        const containerWidth = containerRef.current.clientWidth;
        const textWidth = measureRef.current.offsetWidth;
        const diff = textWidth - containerWidth;
        setOverflowDistance(diff > 3 ? diff : 0);
      }
    };

    // Immediate measure
    measure();

    // ResizeObserver on the container
    let ro: ResizeObserver | null = null;
    if (typeof ResizeObserver !== 'undefined' && containerRef.current) {
      ro = new ResizeObserver(() => measure());
      ro.observe(containerRef.current);
    }

    window.addEventListener('resize', measure);
    return () => {
      if (ro) ro.disconnect();
      window.removeEventListener('resize', measure);
    };
  }, [text, maxWidthClass]);

  const canScroll = overflowDistance > 0;

  // Snappy, smooth 4.0s - 6.5s ping-pong cycle
  const duration = Math.max(4.0, Math.min(6.5, overflowDistance / 15 + 3.5));
  const isScrubbing = canScroll && isHovered && scrubOffset !== null;
  const shouldAnimate = canScroll && (autoScroll || isHovered) && !isScrubbing;

  const handleMouseEnter = () => {
    if (canScroll) {
      setIsHovered(true);
    }
  };

  const handleMouseMove = () => {
    if (canScroll) {
      if (!isHovered) setIsHovered(true);
    }
  };

  const handleMouseLeave = () => {
    setIsHovered(false);
    setScrubOffset(null);
  };

  const handleWheel = (e: React.WheelEvent<HTMLDivElement>) => {
    if (canScroll) {
      const delta = Math.abs(e.deltaX) > 0.5 ? e.deltaX : e.deltaY;
      setScrubOffset((prev) => {
        const current = prev ?? 0;
        const next = current + delta * 0.9;
        return Math.max(0, Math.min(overflowDistance + 12, next));
      });
    }
  };

  // Subtle edge mask: soft 6px fade so letters are not cut off
  const getMaskStyle = () => {
    if (!canScroll) return undefined;
    if (isScrubbing) {
      const atStart = (scrubOffset ?? 0) <= 4;
      const atEnd = (scrubOffset ?? 0) >= overflowDistance + 4;
      if (atStart) {
        return {
          maskImage: 'linear-gradient(to right, black calc(100% - 6px), transparent 100%)',
          WebkitMaskImage: 'linear-gradient(to right, black calc(100% - 6px), transparent 100%)',
        };
      }
      if (atEnd) {
        return {
          maskImage: 'linear-gradient(to right, transparent 0, black 6px)',
          WebkitMaskImage: 'linear-gradient(to right, transparent 0, black 6px)',
        };
      }
      return {
        maskImage: 'linear-gradient(to right, transparent 0, black 6px, black calc(100% - 6px), transparent 100%)',
        WebkitMaskImage: 'linear-gradient(to right, transparent 0, black 6px, black calc(100% - 6px), transparent 100%)',
      };
    }
    return {
      maskImage: 'linear-gradient(to right, black calc(100% - 6px), transparent 100%)',
      WebkitMaskImage: 'linear-gradient(to right, black calc(100% - 6px), transparent 100%)',
    };
  };

  return (
    <div
      ref={containerRef}
      onMouseEnter={handleMouseEnter}
      onMouseMove={handleMouseMove}
      onMouseLeave={handleMouseLeave}
      onWheel={handleWheel}
      className={`overflow-hidden relative inline-flex items-center ${maxWidthClass} ${
        onClick
          ? 'cursor-pointer'
          : canScroll
          ? 'cursor-ew-resize'
          : 'cursor-default'
      } select-text`}
      onClick={onClick}
      style={getMaskStyle()}
      title={title || text}
      data-testid={`marquee-${uniqueId}`}
    >
      {/* Hidden zero-impact measurement span to capture unconstrained text width */}
      <span
        ref={measureRef}
        aria-hidden="true"
        className={`absolute invisible whitespace-nowrap pointer-events-none -z-50 select-none opacity-0 ${className}`}
        style={{ left: -9999, top: -9999 }}
      >
        {text}
      </span>

      {/* Visible interactive text */}
      <span
        style={
          isScrubbing
            ? {
                transform: `translate3d(-${scrubOffset}px, 0, 0)`,
                transition: 'transform 0.08s ease-out',
                willChange: 'transform',
                display: 'inline-block',
                whiteSpace: 'nowrap',
              }
            : shouldAnimate
            ? ({
                '--marquee-dist': `-${overflowDistance + 10}px`,
                animation: `cp-marquee-pingpong ${duration}s ease-in-out infinite`,
                willChange: 'transform',
                display: 'inline-block',
                whiteSpace: 'nowrap',
              } as React.CSSProperties)
            : {
                display: 'inline-block',
                transform: 'translateX(0)',
                transition: 'transform 0.25s ease-out',
              }
        }
        className={`inline-block ${!canScroll ? 'truncate' : 'whitespace-nowrap'} ${className}`}
      >
        {text}
      </span>
    </div>
  );
}
