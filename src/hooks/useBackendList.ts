import { DependencyList, useEffect, useEffectEvent, useState } from "react";

import { useLoadingTask } from "./useLoadingTask";

// Fetches a list from the backend with the loading overlay, refetching when deps change
export function useBackendList<T>(
    fetcher: () => Promise<T[]>,
    deps: DependencyList,
) {
    const withLoading = useLoadingTask();
    const [items, setItems] = useState<T[]>([]);

    const refresh = () => {
        withLoading(fetcher()).then(setItems);
    };

    const onDepsChange = useEffectEvent(refresh);

    useEffect(() => {
        onDepsChange();
        // deps are forwarded from the caller, like useMemo/useEffect themselves
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, deps);

    return [items, setItems, refresh] as const;
}
