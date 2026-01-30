import { splitProps } from 'solid-js';
import "./Input.css";

interface InputProps {
  label?: string;
  error?: string;
  fullWidth?: boolean;
  placeholder?: string;
  value?: string;
  onInput?: (e: InputEvent & { currentTarget: HTMLInputElement }) => void;
  type?: string;
  disabled?: boolean;
}

export function Input(props: InputProps) {
  const [local] = splitProps(props, ['label', 'error', 'fullWidth', 'placeholder', 'value', 'onInput', 'type', 'disabled']);

  const containerClass = () => {
    const classes = ['input'];
    if (local.fullWidth) classes.push('input--full-width');
    if (local.error) classes.push('input--error');
    return classes.join(' ');
  };

  return (
    <div class={containerClass()}>
      {local.label && (
        <label class="input__label">
          {local.label}
        </label>
      )}
      <div class="input__wrapper">
        <input
          class="input__field"
          type={local.type || 'text'}
          placeholder={local.placeholder}
          value={local.value || ''}
          onInput={local.onInput}
          disabled={local.disabled}
        />
      </div>
      {local.error && (
        <span class="input__error">
          {local.error}
        </span>
      )}
    </div>
  );
}
