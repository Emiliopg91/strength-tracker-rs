import { BackendClient } from "@/utils/backend/client";
import { BackendListener } from "@/utils/backend/listener";
import { AppEnvironment } from "@/utils/backend/models";
import { useContext, useEffect, useEffectEvent, useState } from "react";
import { JSX } from "react/jsx-runtime";
import { AppContext, ExerciseOption } from "./AppContext";
import { I18nSettingsContext } from "./I18nSettingsContext";
import { useLoadingTask } from "@/hooks/useLoadingTask";

export function AppProvider({
    children,
    notifyBackend = true,
}: {
    children: JSX.Element;
    notifyBackend?: boolean;
}): JSX.Element {
    const { i18nReady, translate } = useContext(I18nSettingsContext);
    const [environment, setEnvironment] = useState(AppEnvironment.Release);
    const [backendReady, setBackendReady] = useState(false);
    const [backendNotified, setBackendNotified] = useState(false);
    const [sessionsVersion, setSessionsVersion] = useState(0);
    const [rawCatalog, setRawCatalog] = useState<Record<number, number[]>>({});
    const withLoading = useLoadingTask();

    const loadBackendData = useEffectEvent(() => {
        Promise.allSettled([
            withLoading(BackendClient.getEnvironment()).then((env) => {
                setEnvironment(env);

                if (env == AppEnvironment.Release) {
                    document.addEventListener("contextmenu", (e) => {
                        e.preventDefault();
                    });
                }
            }),
            // The exercises table is static, so it is fetched once
            withLoading(BackendClient.getExercisesCatalog())
                .then(setRawCatalog)
                .catch(() => console.warn("Could not load exercises catalog")),
        ]).then(() => {
            setBackendReady(true);
        });
    });

    const notifyFrontendReady = useEffectEvent(() => {
        withLoading(BackendClient.notifyFrontendReady()).then(() => {
            setBackendNotified(true);
        });
    });

    useEffect(() => {
        const unregisterSessionsAdded = BackendListener.onSessionsAdded(() => {
            setSessionsVersion((previous) => previous + 1);
        });

        loadBackendData();

        return () => {
            unregisterSessionsAdded();
        };
    }, []);

    // The backend is notified once both this provider and the i18n one have loaded
    useEffect(() => {
        if (notifyBackend && backendReady && i18nReady) {
            notifyFrontendReady();
        }
    }, [notifyBackend, backendReady, i18nReady]);

    const appReady =
        backendReady && i18nReady && (!notifyBackend || backendNotified);

    const exercisesCatalog: ExerciseOption[] = [];
    const categoryLabels = new Map<number, string>();
    for (const [cat, ex_ids] of Object.entries(rawCatalog)) {
        const ex_cat = Number(cat);
        categoryLabels.set(ex_cat, translate("exercise_" + ex_cat));
        for (const ex_id of ex_ids) {
            exercisesCatalog.push({
                ex_cat,
                ex_id,
                label: translate("exercise_" + ex_cat + "_" + ex_id),
            });
        }
    }
    // Sorted by category first, as Autocomplete's groupBy expects grouped options to be contiguous
    exercisesCatalog.sort(
        (a, b) =>
            categoryLabels
                .get(a.ex_cat)!
                .localeCompare(categoryLabels.get(b.ex_cat)!) ||
            a.label.localeCompare(b.label),
    );

    const value = {
        appReady,
        environment,
        sessionsVersion,
        exercisesCatalog,
    };

    return <AppContext.Provider value={value}>{children}</AppContext.Provider>;
}
