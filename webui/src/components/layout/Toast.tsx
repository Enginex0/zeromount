import { Show, createEffect, createSignal } from 'solid-js';
import { theme } from '../../lib/theme';
import { store } from '../../lib/store';

interface ToastProps {
  message: string;
  type: 'success' | 'error' | 'info';
  visible: boolean;
}

export function Toast(props: ToastProps) {
  const [show, setShow] = createSignal(false);
  const [translateY, setTranslateY] = createSignal(100);

  createEffect(() => {
    if (props.visible) {
      setShow(true);
      setTimeout(() => setTranslateY(0), 10);
    } else {
      setTranslateY(100);
      setTimeout(() => setShow(false), 300);
    }
  });

  const getTypeStyles = () => {
    const t = store.currentTheme();
    switch (props.type) {
      case 'success':
        return `
          background: ${t.colorSuccess};
          box-shadow: 0 8px 24px ${t.colorSuccessGlow};
        `;
      case 'error':
        return `
          background: ${t.colorError};
          box-shadow: 0 8px 24px ${t.colorErrorGlow};
        `;
      default:
        return `
          background: ${t.colorInfo};
          box-shadow: 0 8px 24px ${t.colorInfoGlow};
        `;
    }
  };

  const getIcon = () => {
    switch (props.type) {
      case 'success':
        return (
          <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
            <path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/>
          </svg>
        );
      case 'error':
        return (
          <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
            <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-2h2v2zm0-4h-2V7h2v6z"/>
          </svg>
        );
      default:
        return (
          <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
            <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-6h2v6zm0-8h-2V7h2v2z"/>
          </svg>
        );
    }
  };

  return (
    <Show when={show()}>
      <div
        style={`
          position: fixed;
          bottom: 100px;
          left: 50%;
          transform: translateX(-50%) translateY(${translateY()}px);
          z-index: 1000;
          display: flex;
          align-items: center;
          gap: 10px;
          padding: 14px 20px;
          border-radius: ${theme.radiusLarge};
          color: #FFFFFF;
          font-family: ${theme.fontBody};
          font-size: 14px;
          font-weight: 500;
          transition: transform 0.3s cubic-bezier(0.34, 1.56, 0.64, 1);
          ${getTypeStyles()}
        `}
      >
        {getIcon()}
        {props.message}
      </div>
    </Show>
  );
}
