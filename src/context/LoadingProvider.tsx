import { BackendListener } from "@/utils/backend/listener";
import { useEffect, useState } from "react";
import { JSX } from "react/jsx-runtime";
import { LoadingContext, LoadingStateContext } from "./LoadingContext";

export function LoadingProvider({
    children,
}: {
    children: JSX.Element;
}): JSX.Element {
    const [loadingCount, setLoadingCount] = useState(0);

    const startLoading = () => {
        setLoadingCount((previous) => previous + 1);
    };

    const finishLoading = () => {
        setLoadingCount((previous) => Math.max(0, previous - 1));
    };

    const loading = loadingCount > 0;

    useEffect(() => {
        const unregisterStartLoading = BackendListener.onStartLoading(() => {
            setLoadingCount((previous) => previous + 1);
        });

        const unregisterFinishLoading = BackendListener.onFinishLoading(() => {
            setLoadingCount((previous) => Math.max(0, previous - 1));
        });

        return () => {
            unregisterStartLoading();
            unregisterFinishLoading();
        };
    }, []);

    const actions = { startLoading, finishLoading };

    return (
        <LoadingContext.Provider value={actions}>
            <LoadingStateContext.Provider value={loading}>
                {children}
            </LoadingStateContext.Provider>
        </LoadingContext.Provider>
    );
}
