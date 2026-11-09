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
    const [connectUser, setConnectUser] = useState<string | null>(null);
    const [environment, setEnvironment] = useState(AppEnvironment.Release);
    const [backendNotified, setBackendNotified] = useState(false);
    const [sessionsVersion, setSessionsVersion] = useState(0);
    const [progressionsAvailable, setProgressionsAvailableVal] =
        useState(false);
    const [rawCatalog, setRawCatalog] = useState<Record<number, number[]>>({});
    // Keyed by "<ex_cat>-<ex_id>"; only exercises with non-empty notes
    const [exerciseNotes, setExerciseNotes] = useState<Map<string, string>>(
        () => new Map(),
    );
    const withLoading = useLoadingTask();

    const unsetProgressionsAvailable = () => {
        setProgressionsAvailableVal(false);
    };

    const setProgressionsAvailable = () => {
        setProgressionsAvailableVal(true);
    };

    const updateExerciseNotes = (
        ex_cat: number,
        ex_id: number,
        notes: string,
    ) => {
        setExerciseNotes((previous) => {
            const next = new Map(previous);
            if (notes) next.set(`${ex_cat}-${ex_id}`, notes);
            else next.delete(`${ex_cat}-${ex_id}`);
            return next;
        });
    };

    const loadBackendData = useEffectEvent(() => {
        // Nothing here is needed to use the app; it fills in without the overlay
        BackendClient.getExercisesCatalog()
            .then(setRawCatalog)
            .catch(() => console.warn("Could not load exercises catalog"));

        BackendClient.getEnvironment().then((env) => {
            setEnvironment(env);

            if (env == AppEnvironment.Release) {
                document.addEventListener("contextmenu", (e) => {
                    e.preventDefault();
                });
            }
        });

        BackendClient.getExercises()
            .then((data) => {
                const notes = new Map<string, string>();
                for (const e of data) {
                    if (e.notes) notes.set(`${e.category}-${e.id}`, e.notes);
                }
                setExerciseNotes(notes);
            })
            .catch(() => console.warn("Could not load exercises notes"));

        BackendClient.connectLoggedUser()
            .then(setConnectUser)
            .catch(() => console.warn("Could not get Garmin Connect user"));

        BackendClient.availableAnyProgressionSuggestions()
            .then(setProgressionsAvailableVal)
            .catch(() => console.warn("Could not get exercise progressions"));
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
        const unregisterProgressionAvailable =
            BackendListener.onProgressionsAvailable((val) => {
                setProgressionsAvailableVal(val);
            });
        const unregisterConnectLogged =
            BackendListener.onConnectLogged(setConnectUser);

        loadBackendData();

        return () => {
            unregisterSessionsAdded();
            unregisterConnectLogged();
            unregisterProgressionAvailable();
        };
    }, []);

    // The backend is notified once the i18n provider has loaded
    useEffect(() => {
        if (notifyBackend && i18nReady) {
            notifyFrontendReady();
        }
    }, [notifyBackend, i18nReady]);

    const appReady = i18nReady && (!notifyBackend || backendNotified);

    const exerciseName = (ex_cat: number | null, ex_id: number | null) => {
        if (ex_cat == null || ex_id == null) {
            return "";
        }
        return translate(`exercise_${ex_cat}_${ex_id}`);
    };

    const categoryName = (ex_cat: number) => {
        return translate(`category_${ex_cat}`);
    };

    const exercisesCatalog: ExerciseOption[] = [];
    for (const [cat, ex_ids] of Object.entries(rawCatalog)) {
        const ex_cat = Number(cat);
        for (const ex_id of ex_ids) {
            exercisesCatalog.push({
                ex_cat,
                ex_id,
                label: exerciseName(ex_cat, ex_id),
                notes: exerciseNotes.get(`${ex_cat}-${ex_id}`) ?? null,
            });
        }
    }
    exercisesCatalog.sort((a, b) => a.label.localeCompare(b.label));

    const value = {
        appReady,
        environment,
        sessionsVersion,
        exercisesCatalog,
        updateExerciseNotes,
        exerciseName,
        categoryName,
        connectUser,
        progressionsAvailable,
        unsetProgressionsAvailable,
        setProgressionsAvailable,
    };

    return <AppContext.Provider value={value}>{children}</AppContext.Provider>;
}
