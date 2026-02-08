import { Show, createEffect, createSignal, onCleanup } from 'solid-js';
import type { JSXElement } from 'solid-js';
import { theme } from '../../lib/theme';
import { store } from '../../lib/store';

interface ModalProps {
  open: boolean;
  onClose: () => void;
  title: string;
  children?: JSXElement;
}

export function Modal(props: ModalProps) {
  const [visible, setVisible] = createSignal(false);
  const [translateY, setTranslateY] = createSignal(100);
  const [backdropOpacity, setBackdropOpacity] = createSignal(0);

  createEffect(() => {
    if (props.open) {
      setVisible(true);
      setTimeout(() => {
        setTranslateY(0);
        setBackdropOpacity(1);
      }, 10);

      const handleKeyDown = (e: KeyboardEvent) => {
        if (e.key === 'Escape') {
          props.onClose();
        }
      };
      document.addEventListener('keydown', handleKeyDown);
      onCleanup(() => document.removeEventListener('keydown', handleKeyDown));
    } else {
      setTranslateY(100);
      setBackdropOpacity(0);
      setTimeout(() => setVisible(false), 300);
    }
  });

  return (
    <Show when={visible()}>
      {/* Backdrop */}
      <div
        onClick={props.onClose}
        style={`
          position: fixed;
          inset: 0;
          background: rgba(0, 0, 0, 0.6);
          backdrop-filter: blur(4px);
          -webkit-backdrop-filter: blur(4px);
          z-index: 200;
          opacity: ${backdropOpacity()};
          transition: opacity 0.3s ease;
        `}
      />

      {/* Modal Content */}
      <div
        style={`
          position: fixed;
          bottom: 0;
          left: 0;
          right: 0;
          background: ${store.currentTheme().gradientSecondary};
          border-top-left-radius: ${theme.radiusXLarge};
          border-top-right-radius: ${theme.radiusXLarge};
          padding: 24px 20px;
          padding-bottom: calc(24px + env(safe-area-inset-bottom));
          z-index: 201;
          max-height: 85vh;
          overflow-y: auto;
          transform: translateY(${translateY()}%);
          transition: transform 0.3s cubic-bezier(0.34, 1.56, 0.64, 1);
        `}
      >
        {/* Handle */}
        <div
          style={`
            width: 40px;
            height: 4px;
            background: ${store.currentTheme().textTertiary};
            border-radius: 2px;
            margin: 0 auto 20px;
          `}
        />

        {/* Title */}
        <h2
          style={`
            font-family: ${theme.fontDisplay};
            font-size: 24px;
            font-weight: 700;
            text-align: center;
            background: ${store.currentTheme().gradientPrimary};
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
            background-clip: text;
            margin: 0 0 24px;
          `}
        >
          {props.title}
        </h2>

        {props.children}
      </div>
    </Show>
  );
}
