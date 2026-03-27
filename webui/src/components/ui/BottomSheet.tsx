import { Show, For, createEffect, createSignal, onCleanup } from 'solid-js';
import { Portal } from 'solid-js/web';
import type { JSXElement } from 'solid-js';
import { t } from '../../lib/i18n';
import './BottomSheet.css';

export interface SheetOption {
  value: string;
  label: string;
  description?: string;
  icon?: JSXElement;
  disabled?: boolean;
}

interface BottomSheetProps {
  open: boolean;
  onClose: () => void;
  title: string;
  options: SheetOption[];
  value: string;
  onChange: (value: string) => void;
  customInput?: {
    placeholder: string;
    value: string;
    onInput: (value: string) => void;
    onConfirm: (value: string) => void;
  };
}

export function BottomSheet(props: BottomSheetProps) {
  const [visible, setVisible] = createSignal(false);
  const [animating, setAnimating] = createSignal(false);
  const [customMode, setCustomMode] = createSignal(false);

  createEffect(() => {
    if (props.open) {
      setCustomMode(false);
      setVisible(true);
      requestAnimationFrame(() => setAnimating(true));
      document.body.style.overflow = 'hidden';
      onCleanup(() => { document.body.style.overflow = ''; });
    } else {
      setAnimating(false);
      const timer = setTimeout(() => setVisible(false), 320);
      document.body.style.overflow = '';
      onCleanup(() => clearTimeout(timer));
    }
  });

  const handleSelect = (value: string) => {
    if (value === 'custom') {
      setCustomMode(true);
      props.onChange(value);
      return;
    }
    setCustomMode(false);
    props.onChange(value);
    props.onClose();
  };

  const isCustomActive = () => customMode() || props.value === 'custom' || (
    props.customInput && !props.options.some(o => o.value === props.value && o.value !== 'custom')
  );

  return (
    <Show when={visible()}>
      <Portal>
        <div class={`sheet-backdrop${animating() ? ' sheet-backdrop--visible' : ''}`} onClick={props.onClose} />

        <div class={`sheet${animating() ? ' sheet--visible' : ''}${isCustomActive() ? ' sheet--input-mode' : ''}`}>
          <div class="sheet__handle" />

          <div class="sheet__title">{props.title}</div>

          <Show when={!isCustomActive()}>
            <div class="sheet__options">
              <For each={props.options}>
                {(option) => {
                  const selected = () => option.value === 'custom' ? false : props.value === option.value;
                  return (
                    <button
                      class={`sheet__option${selected() ? ' sheet__option--selected' : ''}${option.disabled ? ' sheet__option--disabled' : ''}`}
                      onClick={() => !option.disabled && handleSelect(option.value)}
                      disabled={option.disabled}
                    >
                      <Show when={option.icon}>
                        <div class="sheet__option-icon">{option.icon}</div>
                      </Show>
                      <div class="sheet__option-content">
                        <div class="sheet__option-label">{option.label}</div>
                        <Show when={option.description}>
                          <div class="sheet__option-desc">{option.description}</div>
                        </Show>
                      </div>
                      <div class={`sheet__option-check${selected() ? ' sheet__option-check--visible' : ''}`}>
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round">
                          <polyline points="20 6 9 17 4 12" />
                        </svg>
                      </div>
                    </button>
                  );
                }}
              </For>
            </div>
          </Show>

          <Show when={props.customInput && isCustomActive()}>
            <button class="sheet__back" onClick={() => setCustomMode(false)}>
              <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><path d="M20 11H7.83l5.59-5.59L12 4l-8 8 8 8 1.41-1.41L7.83 13H20v-2z"/></svg>
              {t('ui.backToOptions')}
            </button>
            <div class="sheet__custom">
              <input
                ref={(el) => setTimeout(() => el.focus(), 100)}
                class="sheet__custom-input"
                type="text"
                placeholder={props.customInput!.placeholder}
                value={props.customInput!.value}
                onInput={(e) => props.customInput!.onInput(e.currentTarget.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && props.customInput!.value.trim()) {
                    props.customInput!.onConfirm(props.customInput!.value.trim());
                    props.onClose();
                  }
                }}
              />
              <button
                class="sheet__custom-confirm"
                onClick={() => {
                  if (props.customInput!.value.trim()) {
                    props.customInput!.onConfirm(props.customInput!.value.trim());
                    props.onClose();
                  }
                }}
                disabled={!props.customInput!.value.trim()}
              >
                <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
                  <path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/>
                </svg>
              </button>
            </div>
          </Show>
        </div>
      </Portal>
    </Show>
  );
}
