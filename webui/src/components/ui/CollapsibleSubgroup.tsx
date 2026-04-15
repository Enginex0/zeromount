import { createSignal, Show, type JSX } from 'solid-js';
import { t } from '../../lib/i18n';
import './CollapsibleSubgroup.css';

interface CollapsibleSubgroupProps {
  label: string;
  hiddenCount: number;
  defaultItems: JSX.Element;
  expandedItems?: JSX.Element;
}

export function CollapsibleSubgroup(props: CollapsibleSubgroupProps) {
  const [expanded, setExpanded] = createSignal(false);
  const hasExpandable = () => props.hiddenCount > 0 && props.expandedItems;

  return (
    <>
      <div
        class={`subgroup__header${hasExpandable() ? '' : ' subgroup__header--static'}`}
        onClick={() => hasExpandable() && setExpanded(!expanded())}
      >
        <span class="subgroup__label">{props.label}</span>
        <Show when={hasExpandable()}>
          <div class="subgroup__meta">
            <Show when={!expanded()}>
              <span class="subgroup__count">{t('ui.nMore', { count: props.hiddenCount })}</span>
            </Show>
            <svg
              class={`subgroup__chevron${expanded() ? ' subgroup__chevron--open' : ''}`}
              width="24" height="24" viewBox="0 0 24 24" fill="currentColor"
            >
              <path d="M7 10l5 5 5-5z" />
            </svg>
          </div>
        </Show>
      </div>
      {props.defaultItems}
      <Show when={expanded() && hasExpandable()}>
        <div class="subgroup__expanded">
          {props.expandedItems}
        </div>
      </Show>
    </>
  );
}
