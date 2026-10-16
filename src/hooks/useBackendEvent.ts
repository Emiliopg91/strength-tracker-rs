import { useEffect, useRef } from "react";

// Subscribes once to a BackendListener event, always invoking the latest handler
export function useBackendEvent<T>(
    subscribe: (callback: (payload: T) => void) => () => void,
    handler: (payload: T) => void,
) {
    const handlerRef = useRef(handler);
    handlerRef.current = handler;

    useEffect(() => {
        return subscribe((payload) => handlerRef.current(payload));
    }, [subscribe]);
}
