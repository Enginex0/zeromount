import { splitProps } from 'solid-js';
import "./Input.css";

interface InputProps {
  fullWidth?: boolean;
  placeholder?: string;
  value?: string;
  onInput?: (e: InputEvent & { currentTarget: HTMLInputElement }) => void;
  onBlur?: (e: FocusEvent & { currentTarget: HTMLInputElement }) => void;
  type?: string;
  disabled?: boolean;
}

export function Input(props: InputProps) {
  const [local] = splitProps(props, ['fullWidth', 'placeholder', 'value', 'onInput', 'onBlur', 'type', 'disabled']);

  const containerClass = () => {
    const classes = ['input'];
    if (local.fullWidth) classes.push('input--full-width');
    return classes.join(' ');
  };

  return (
    <div class={containerClass()}>
      <div class="input__wrapper">
        <input
          class="input__field"
          type={local.type || 'text'}
          placeholder={local.placeholder}
          value={local.value || ''}
          onInput={local.onInput}
          onBlur={local.onBlur}
          disabled={local.disabled}
        />
      </div>
    </div>
  );
}
