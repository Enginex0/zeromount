import { createSignal, onMount, Show, Switch, Match } from 'solid-js';
import { Header } from './components/layout/Header';
import { NavBar } from './components/layout/NavBar';
import { Toast } from './components/layout/Toast';
import { StatusTab } from './routes/StatusTab';
import { ModulesTab } from './routes/ModulesTab';
import { ConfigTab } from './routes/ConfigTab';
import { SettingsTab } from './routes/SettingsTab';
import { store } from './lib/store';

export function App() {
  const [isReady, setIsReady] = createSignal(false);

  onMount(async () => {
    await store.loadInitialData();
    setIsReady(true);
  });

  return (
    <Show
      when={isReady()}
      fallback={
        <div
          style={`
            min-height: 100vh;
            min-height: 100dvh;
            display: flex;
            align-items: center;
            justify-content: center;
            background: ${store.currentTheme().bgPrimary};
            color: ${store.currentTheme().textPrimary};
            font-family: ${store.currentTheme().fontBody};
          `}
        >
          Loading...
        </div>
      }
    >
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
              visible={true}
            />
          )}
        </Show>
      </div>
    </Show>
  );
}
