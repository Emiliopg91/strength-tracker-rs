import { LoadingContext } from "@/context/LoadingContext";
import { useCallback, useContext } from "react";

// Shows the global loading overlay while the given promise is pending
export function useLoadingTask() {
    const { startLoading, finishLoading } = useContext(LoadingContext);

    return useCallback(
        <T>(promise: Promise<T>): Promise<T> => {
            startLoading();
            return promise.finally(finishLoading);
        },
        [startLoading, finishLoading],
    );
}
