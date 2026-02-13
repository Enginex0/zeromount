import { onMount, Show, Switch, Match } from 'solid-js';
import { Header } from './components/layout/Header';
import { NavBar } from './components/layout/NavBar';
import { Toast } from './components/layout/Toast';
import { StatusTab } from './routes/StatusTab';
import { ModulesTab } from './routes/ModulesTab';
import { ConfigTab } from './routes/ConfigTab';
import { SettingsTab } from './routes/SettingsTab';
import { store } from './lib/store';

export function App() {
  onMount(() => {
    globalThis.ksu?.enableEdgeToEdge?.(true);
    store.loadInitialData();
  });

  return (
    <div
      style={`
        min-height: 100vh;
        min-height: 100dvh;
        background: ${store.currentTheme().bgPrimary};
        color: ${store.currentTheme().textPrimary};
        font-family: ${store.currentTheme().fontBody};
        overflow-x: hidden;
      `}
    >
      <Header />

      <main
        style={`
          padding-bottom: ${store.settings.fixedNav ? 'calc(100px + 48px + env(safe-area-inset-bottom))' : 'calc(100px + env(safe-area-inset-bottom))'};
        `}
      >
        <Switch>
          <Match when={store.activeTab() === 'status'}>
            <StatusTab />
          </Match>
          <Match when={store.activeTab() === 'modules'}>
            <ModulesTab />
          </Match>
          <Match when={store.activeTab() === 'config'}>
            <ConfigTab />
          </Match>
          <Match when={store.activeTab() === 'settings'}>
            <SettingsTab />
          </Match>
        </Switch>
      </main>

      <NavBar
        activeTab={store.activeTab()}
        onTabChange={store.setActiveTab}
      />

      <Show when={store.toast()}>
        {(toast) => (
          <Toast
            message={toast().message}
            type={toast().type}
            duration={toast().duration}
            visible={true}
          />
        )}
      </Show>
    </div>
  );
}
