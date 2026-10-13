import { BackendClient } from "@/utils/backend/client";
import { BackendListener } from "@/utils/backend/listener";
import { AppEnvironment } from "@/utils/backend/models";
import { useContext, useEffect, useMemo, useState } from "react";
import { JSX } from "react/jsx-runtime";
import { AppContext, ExerciseOption } from "./AppContext";
import { I18nSettingsContext } from "./I18nSettingsContext";

export function AppProvider({
  children,
}: {
  children: JSX.Element;
}): JSX.Element {
  const { i18nReady, translate } = useContext(I18nSettingsContext);
  const [environment, setEnvironment] = useState(AppEnvironment.Release);
  const [backendReady, setBackendReady] = useState(false);
  const [appReady, setAppReady] = useState(false);
  const [sessionsVersion, setSessionsVersion] = useState(0);
  const [rawCatalog, setRawCatalog] = useState<Record<number, number[]>>({});

  useEffect(() => {
    const unregisterSessionsAdded = BackendListener.onSessionsAdded(() => {
      setSessionsVersion((previous) => previous + 1);
    });

    BackendClient.getEnvironment()
      .then((env) => {
        setEnvironment(env);

        if (env == AppEnvironment.Release) {
          document.addEventListener("contextmenu", (e) => {
            e.preventDefault();
          });
        }
      })
      .finally(() => {
        setBackendReady(true);
      });

    // The exercises table is static, so it is fetched once
    BackendClient.getExercisesCatalog()
      .then(setRawCatalog)
      .catch(() => console.warn("Could not load exercises catalog"));

    return () => {
      unregisterSessionsAdded();
    };
  }, []);

  // The backend is notified once both this provider and the i18n one have loaded
  useEffect(() => {
    if (backendReady && i18nReady) {
      BackendClient.notifyFrontendReady().then(() => {
        setAppReady(true);
      });
    }
  }, [backendReady, i18nReady]);

  const exercisesCatalog = useMemo<ExerciseOption[]>(() => {
    const catalog: ExerciseOption[] = [];
    const categoryLabels = new Map<number, string>();
    for (const [cat, ex_ids] of Object.entries(rawCatalog)) {
      const ex_cat = Number(cat);
      categoryLabels.set(ex_cat, translate("exercise_" + ex_cat));
      for (const ex_id of ex_ids) {
        catalog.push({
          ex_cat,
          ex_id,
          label: translate("exercise_" + ex_cat + "_" + ex_id),
        });
      }
    }
    // Sorted by category first, as Autocomplete's groupBy expects grouped options to be contiguous
    return catalog.sort(
      (a, b) =>
        categoryLabels
          .get(a.ex_cat)!
          .localeCompare(categoryLabels.get(b.ex_cat)!) ||
        a.label.localeCompare(b.label),
    );
  }, [rawCatalog, translate]);

  const value = useMemo(
    () => ({
      appReady,
      environment,
      sessionsVersion,
      exercisesCatalog,
    }),
    [appReady, environment, sessionsVersion, exercisesCatalog],
  );

  return <AppContext.Provider value={value}>{children}</AppContext.Provider>;
}
