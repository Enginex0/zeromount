import { splitProps } from 'solid-js';
import type { JSXElement } from 'solid-js';
import './Badge.css';

interface BadgeProps {
  variant?: 'default' | 'success' | 'warning' | 'error' | 'info';
  size?: 'small' | 'medium';
  children?: JSXElement;
}

export function Badge(props: BadgeProps) {
  const [local] = splitProps(props, ['variant', 'size', 'children']);

  const variant = () => local.variant || 'default';
  const size = () => local.size || 'medium';

  const className = () => `badge badge--${size()} badge--${variant()}`;

  return (
    <span class={className()}>
      {local.children}
    </span>
  );
}
