import { splitProps } from 'solid-js';
import type { JSXElement } from 'solid-js';
import './Card.css';

interface CardProps {
  variant?: 'glass' | 'elevated' | 'gradient-border';
  padding?: 'none' | 'small' | 'medium' | 'large';
  hoverable?: boolean;
  style?: string;
  onClick?: (e: MouseEvent) => void;
  children?: JSXElement;
}

export function Card(props: CardProps) {
  const [local] = splitProps(props, ['variant', 'padding', 'hoverable', 'children', 'style', 'onClick']);

  const variant = () => local.variant || 'glass';
  const padding = () => local.padding || 'medium';

  const className = () => {
    const classes = ['card'];
    classes.push(`card--${variant()}`);
    classes.push(`card--padding-${padding()}`);
    if (local.hoverable) classes.push('card--hoverable');
    return classes.join(' ');
  };

  return (
    <div
      class={className()}
      onClick={local.onClick}
      style={local.style || undefined}
    >
      {local.children}
    </div>
  );
}
