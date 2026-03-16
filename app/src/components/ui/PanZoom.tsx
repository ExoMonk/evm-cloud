import { useState, useRef, useCallback, type ReactNode } from "react";

interface Props {
  children: ReactNode;
  className?: string;
}

/**
 * Lightweight pan-zoom container.
 * - Scroll/pinch to zoom (0.5x – 3x)
 * - Click+drag to pan
 * - Double-click to reset
 */
export function PanZoom({ children, className = "" }: Props) {
  const [offset, setOffset] = useState({ x: 0, y: 0 });
  const [scale, setScale] = useState(1);
  const dragging = useRef(false);
  const lastPos = useRef({ x: 0, y: 0 });

  const onWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault();
    const delta = e.deltaY * -0.002;
    setScale((s) => Math.min(3, Math.max(0.5, s + delta)));
  }, []);

  const onMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button !== 0) return;
    dragging.current = true;
    lastPos.current = { x: e.clientX, y: e.clientY };
  }, []);

  const onMouseMove = useCallback((e: React.MouseEvent) => {
    if (!dragging.current) return;
    const dx = e.clientX - lastPos.current.x;
    const dy = e.clientY - lastPos.current.y;
    lastPos.current = { x: e.clientX, y: e.clientY };
    setOffset((o) => ({ x: o.x + dx, y: o.y + dy }));
  }, []);

  const onMouseUp = useCallback(() => {
    dragging.current = false;
  }, []);

  const reset = useCallback(() => {
    setOffset({ x: 0, y: 0 });
    setScale(1);
  }, []);

  return (
    <div
      className={`overflow-hidden cursor-grab active:cursor-grabbing ${className}`}
      onWheel={onWheel}
      onMouseDown={onMouseDown}
      onMouseMove={onMouseMove}
      onMouseUp={onMouseUp}
      onMouseLeave={onMouseUp}
      onDoubleClick={reset}
    >
      <div
        className="origin-center transition-transform duration-75"
        style={{
          transform: `translate(${offset.x}px, ${offset.y}px) scale(${scale})`,
        }}
      >
        {children}
      </div>
    </div>
  );
}
