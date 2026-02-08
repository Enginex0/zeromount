import './glass-toggle.css';

interface ToggleProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}

export function Toggle(props: ToggleProps) {
  const handleClick = () => {
    if (props.disabled) return;
    props.onChange(!props.checked);
  };

  return (
    <div
      class={`custom-toggle${props.disabled ? ' custom-toggle--disabled' : ''}`}
      onClick={handleClick}
      role="switch"
      aria-checked={props.checked}
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          handleClick();
        }
      }}
    >
      <div class={`toggle-track${props.checked ? ' toggle-track--active' : ''}`}>
        <div class="toggle-thumb" />
      </div>
    </div>
  );
}
