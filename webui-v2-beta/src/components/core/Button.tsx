import { splitProps, Show } from 'solid-js';
import type { JSXElement } from 'solid-js';
import { needsDarkText } from '../../lib/theme';
import { store } from '../../lib/store';
import './Button.css';

interface ButtonProps {
  variant?: 'primary' | 'secondary' | 'danger' | 'ghost';
  size?: 'small' | 'medium' | 'large';
  loading?: boolean;
  fullWidth?: boolean;
  disabled?: boolean;
  onClick?: (e: MouseEvent) => void;
  style?: string;
  type?: 'button' | 'submit' | 'reset';
  class?: string;
  children?: JSXElement;
}

export function Button(props: ButtonProps) {
  const [local] = splitProps(props, ['variant', 'size', 'loading', 'fullWidth', 'children', 'disabled', 'onClick', 'style', 'type', 'class']);

  const variant = () => local.variant || 'primary';
  const size = () => local.size || 'medium';

  const classNames = () => {
    const classes = ['button', `button--${variant()}`, `button--${size()}`];
    if (local.fullWidth) classes.push('button--full-width');
    if (local.class) classes.push(local.class);
    return classes.join(' ');
  };

  const dynamicStyles = () => {
    if (variant() === 'primary') {
      const textColor = needsDarkText(store.settings.accentColor) ? '#1A1A2E' : '#FFFFFF';
      return `color: ${textColor};`;
    }
    return '';
  };

  return (
    <button
      type={local.type || 'button'}
      class={classNames()}
      disabled={local.loading || local.disabled}
      onClick={local.onClick}
      style={`${dynamicStyles()} ${local.style || ''}`}
    >
      <Show when={local.loading}>
        <span class="button__spinner" />
      </Show>
      <Show when={!local.loading}>
        {local.children}
      </Show>
    </button>
  );
}
