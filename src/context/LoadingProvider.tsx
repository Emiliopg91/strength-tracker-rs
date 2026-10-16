import { BackendListener } from "@/utils/backend/listener";
import { useCallback, useEffect, useMemo, useState } from "react";
import { JSX } from "react/jsx-runtime";
import { LoadingContext, LoadingStateContext } from "./LoadingContext";

export function LoadingProvider({
    children,
}: {
    children: JSX.Element;
}): JSX.Element {
    const [loadingCount, setLoadingCount] = useState(0);

    const startLoading = useCallback(() => {
        setLoadingCount((previous) => previous + 1);
    }, []);

    const finishLoading = useCallback(() => {
        setLoadingCount((previous) => Math.max(0, previous - 1));
    }, []);

    const loading = loadingCount > 0;

    useEffect(() => {
        const unregisterStartLoading = BackendListener.onStartLoading(() => {
            startLoading();
        });

        const unregisterFinishLoading = BackendListener.onFinishLoading(() => {
            finishLoading();
        });

        return () => {
            unregisterStartLoading();
            unregisterFinishLoading();
        };
    }, [startLoading, finishLoading]);

    const actions = useMemo(
        () => ({ startLoading, finishLoading }),
        [startLoading, finishLoading],
    );

    return (
        <LoadingContext.Provider value={actions}>
            <LoadingStateContext.Provider value={loading}>
                {children}
            </LoadingStateContext.Provider>
        </LoadingContext.Provider>
    );
}
