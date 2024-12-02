import { Toasts } from "App/Common/Toast";
import { AppLayout } from "App/Layout/AppLayout";
import { createBrowserHistory } from "history";
import { useExtensionEvents } from "hooks/useExtensionEvents";
import { useRegistryFeatures } from "hooks/useRegistryFeatures";
import { useServerSideEvents } from "hooks/useServerSideEvents";
import { useTransactionCallback } from "hooks/useTransactionCallbacks";
import { useTransactionNotifications } from "hooks/useTransactionNotifications";
import { useTransactionWatcher } from "hooks/useTransactionWatcher";
import { Outlet } from "react-router-dom";
import { ChainLoader } from "./Setup/ChainLoader";
import { WorkerTest } from "./WorkerTest";

export const history = createBrowserHistory({ window });

export function App(): JSX.Element {
  useExtensionEvents();
  useTransactionNotifications();
  useTransactionCallback();
  useRegistryFeatures();
  useTransactionWatcher();
  useServerSideEvents();

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (window as any).spendingKey = async () => {};

  return (
    <>
      <Toasts />
      <AppLayout>
        <WorkerTest></WorkerTest>
        <ChainLoader>
          <Outlet />
        </ChainLoader>
      </AppLayout>
    </>
  );
}
