import { createEffect, createSignal, For } from 'solid-js';
import { store } from '../../lib/store';
import type { Tab } from '../../lib/types';
import "./NavBar.css";

interface NavBarProps {
  activeTab: Tab;
  onTabChange: (tab: Tab) => void;
}

const tabs: { id: Tab; label: string; icon: string }[] = [
  { id: 'status', label: 'Status', icon: 'power_settings_new' },
  { id: 'modules', label: 'Modules', icon: 'folder' },
  { id: 'config', label: 'Config', icon: 'tune' },
  { id: 'settings', label: 'Settings', icon: 'settings' },
];

export function NavBar(props: NavBarProps) {
  const [indicatorLeft, setIndicatorLeft] = createSignal(0);
  const [indicatorWidth, setIndicatorWidth] = createSignal(0);
  const [isStretching, setIsStretching] = createSignal(false);

  let tabRefs: { [key: string]: HTMLButtonElement | undefined } = {};

  createEffect(() => {
    const activeTabEl = tabRefs[props.activeTab];
    if (activeTabEl) {
      const rect = activeTabEl.getBoundingClientRect();
      const parentRect = activeTabEl.parentElement?.getBoundingClientRect();
      if (parentRect) {
        setIsStretching(true);
        setTimeout(() => {
          setIndicatorLeft(rect.left - parentRect.left);
          setIndicatorWidth(rect.width);
          setTimeout(() => setIsStretching(false), 200);
        }, 50);
      }
    }
  });

  const extraPadding = () => store.settings.fixedNav;

  return (
    <nav class={`navbar ${extraPadding() ? 'navbar--fixed-nav' : ''}`}>
      <div class="navbar__tabs">
        <div
          class={`navbar__indicator ${isStretching() ? 'navbar__indicator--stretching' : ''}`}
          style={{
            left: `${indicatorLeft()}px`,
            width: `${indicatorWidth()}px`,
          }}
        />

        <For each={tabs}>
          {(tab) => (
            <button
              ref={(el) => (tabRefs[tab.id] = el)}
              onClick={() => props.onTabChange(tab.id)}
              class={`navbar__tab ${props.activeTab === tab.id ? 'navbar__tab--active' : ''}`}
            >
              <span class="navbar__icon">
                {tab.icon === 'power_settings_new' && (
                  <svg width="24" height="24" viewBox="0 0 24 24" fill="currentColor">
                    <path d="M13 3h-2v10h2V3zm4.83 2.17l-1.42 1.42C17.99 7.86 19 9.81 19 12c0 3.87-3.13 7-7 7s-7-3.13-7-7c0-2.19 1.01-4.14 2.58-5.42L6.17 5.17C4.23 6.82 3 9.26 3 12c0 4.97 4.03 9 9 9s9-4.03 9-9c0-2.74-1.23-5.18-3.17-6.83z"/>
                  </svg>
                )}
                {tab.icon === 'folder' && (
                  <svg width="24" height="24" viewBox="0 0 24 24" fill="currentColor">
                    <path d="M10 4H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2h-8l-2-2z"/>
                  </svg>
                )}
                {tab.icon === 'tune' && (
                  <svg width="24" height="24" viewBox="0 0 24 24" fill="currentColor">
                    <path d="M3 17v2h6v-2H3zM3 5v2h10V5H3zm10 16v-2h8v-2h-8v-2h-2v6h2zM7 9v2H3v2h4v2h2V9H7zm14 4v-2H11v2h10zm-6-4h2V7h4V5h-4V3h-2v6z"/>
                  </svg>
                )}
                {tab.icon === 'settings' && (
                  <svg width="24" height="24" viewBox="0 0 24 24" fill="currentColor">
                    <path d="M19.14 12.94c.04-.31.06-.63.06-.94 0-.31-.02-.63-.06-.94l2.03-1.58c.18-.14.23-.41.12-.61l-1.92-3.32c-.12-.22-.37-.29-.59-.22l-2.39.96c-.5-.38-1.03-.7-1.62-.94l-.36-2.54c-.04-.24-.24-.41-.48-.41h-3.84c-.24 0-.43.17-.47.41l-.36 2.54c-.59.24-1.13.57-1.62.94l-2.39-.96c-.22-.08-.47 0-.59.22L2.74 8.87c-.12.21-.08.47.12.61l2.03 1.58c-.04.31-.06.63-.06.94s.02.63.06.94l-2.03 1.58c-.18.14-.23.41-.12.61l1.92 3.32c.12.22.37.29.59.22l2.39-.96c.5.38 1.03.7 1.62.94l.36 2.54c.05.24.24.41.48.41h3.84c.24 0 .44-.17.47-.41l.36-2.54c.59-.24 1.13-.56 1.62-.94l2.39.96c.22.08.47 0 .59-.22l1.92-3.32c.12-.22.07-.47-.12-.61l-2.01-1.58zM12 15.6c-1.98 0-3.6-1.62-3.6-3.6s1.62-3.6 3.6-3.6 3.6 1.62 3.6 3.6-1.62 3.6-3.6 3.6z"/>
                  </svg>
                )}
              </span>

              <span class="navbar__label">
                {tab.label}
              </span>
            </button>
          )}
        </For>
      </div>
    </nav>
  );
}
