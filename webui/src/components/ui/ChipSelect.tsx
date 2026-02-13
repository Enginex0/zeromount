import { For } from 'solid-js';
import './ChipSelect.css';

export interface ChipOption {
  value: string;
  label: string;
  disabled?: boolean;
}

interface ChipSelectProps {
  value: string;
  onChange: (value: string) => void;
  options: ChipOption[];
}

export function ChipSelect(props: ChipSelectProps) {
  return (
    <div class="chips">
      <For each={props.options}>
        {(option) => (
          <button
            class={`chip${props.value === option.value ? ' chip--selected' : ''}${option.disabled ? ' chip--disabled' : ''}`}
            onClick={() => !option.disabled && props.onChange(option.value)}
            disabled={option.disabled}
          >
            {option.label}
          </button>
        )}
      </For>
    </div>
  );
}
