import { onMount, Show, Switch, Match, createMemo, lazy, Suspense } from 'solid-js';
import { Header } from './components/layout/Header';
import { NavBar } from './components/layout/NavBar';
import { Toast } from './components/layout/Toast';
import { StatusTab } from './routes/StatusTab';
import { store } from './lib/store';
import bgClouds from './assets/bg-clouds.webp';

const ModulesTab = lazy(() => import('./routes/ModulesTab').then(m => ({ default: m.ModulesTab })));
const ConfigTab = lazy(() => import('./routes/ConfigTab').then(m => ({ default: m.ConfigTab })));
const SettingsTab = lazy(() => import('./routes/SettingsTab').then(m => ({ default: m.SettingsTab })));

export function App() {
  onMount(() => {
    globalThis.ksu?.enableEdgeToEdge?.(true);
    store.loadInitialData();
    document.documentElement.style.setProperty('--bg-opacity', String(store.bgOpacity()));

    // Preload remaining tabs while user views Status
    requestIdleCallback(() => {
      import('./routes/ModulesTab');
      import('./routes/ConfigTab');
      import('./routes/SettingsTab');
    });
  });

  const wrapperBg = createMemo(() => {
    const op = store.bgOpacity();
    if (op <= 0) return store.currentTheme().bgPrimary;
    const scrim = 1 - op * 0.85;
    return `rgba(${store.currentTheme().bgBase}, ${scrim})`;
  });

  return (
    <>
      <Show when={store.bgOpacity() > 0}>
        <div class="app-bg-image" style={`background-image: url(${bgClouds})`} />
      </Show>

      <div
        class="app-content"
        style={`
          min-height: 100vh;
          min-height: 100dvh;
          background: ${wrapperBg()};
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
          <Suspense>
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
          </Suspense>
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
    </>
  );
}
