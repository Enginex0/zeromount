import { createSignal, createEffect } from 'solid-js';
import { store } from '../../lib/store';

interface ToggleProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}

export function Toggle(props: ToggleProps) {
  const [pressing, setPressing] = createSignal(false);
  const [thumbPosition, setThumbPosition] = createSignal(props.checked ? 28 : 2);
  const [thumbWidth, setThumbWidth] = createSignal(24);

  createEffect(() => {
    setThumbPosition(props.checked ? 28 : 2);
  });

  const handleClick = () => {
    if (props.disabled) return;

    // Stretch animation on click
    setThumbWidth(28);
    setTimeout(() => {
      setThumbWidth(24);
      props.onChange(!props.checked);
    }, 100);
  };

  const t = () => store.currentTheme();

  return (
    <button
      type="button"
      onClick={handleClick}
      disabled={props.disabled}
      onMouseDown={() => setPressing(true)}
      onMouseUp={() => setPressing(false)}
      onMouseLeave={() => setPressing(false)}
      style={`
        position: relative;
        width: 56px;
        height: 28px;
        border-radius: 14px;
        border: none;
        cursor: ${props.disabled ? 'not-allowed' : 'pointer'};
        background: ${props.checked ? t().gradientPrimary : t().bgSurfaceElevated};
        transition: background 0.3s ease;
        opacity: ${props.disabled ? '0.5' : '1'};
        padding: 0;
        outline: none;
      `}
    >
      {/* Glow effect when active */}
      <div
        style={`
          position: absolute;
          inset: -4px;
          border-radius: 18px;
          background: ${props.checked ? 'rgba(var(--accent-rgb), 0.2)' : 'transparent'};
          filter: blur(8px);
          transition: all 0.3s ease;
          pointer-events: none;
        `}
      />

      {/* Thumb */}
      <div
        style={`
          position: absolute;
          top: 2px;
          left: ${thumbPosition()}px;
          width: ${thumbWidth()}px;
          height: 24px;
          border-radius: 12px;
          background: ${t().textPrimary};
          box-shadow: ${t().shadowSmall};
          transition: left 0.2s cubic-bezier(0.34, 1.56, 0.64, 1), width 0.1s ease;
          transform: ${pressing() ? 'scale(0.95)' : 'scale(1)'};
        `}
      />
    </button>
  );
}
