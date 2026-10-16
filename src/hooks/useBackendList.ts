import {
    DependencyList,
    useCallback,
    useEffect,
    useRef,
    useState,
} from "react";

import { useLoadingTask } from "./useLoadingTask";

// Fetches a list from the backend with the loading overlay, refetching when deps change
export function useBackendList<T>(
    fetcher: () => Promise<T[]>,
    deps: DependencyList,
) {
    const withLoading = useLoadingTask();
    const [items, setItems] = useState<T[]>([]);

    const fetcherRef = useRef(fetcher);
    fetcherRef.current = fetcher;

    const refresh = useCallback(() => {
        withLoading(fetcherRef.current()).then(setItems);
    }, [withLoading]);

    useEffect(() => {
        refresh();
    }, deps);

    return [items, setItems, refresh] as const;
}
