import { useEffect, useEffectEvent } from "react";

// Subscribes once to a BackendListener event, always invoking the latest handler
export function useBackendEvent<T>(
    subscribe: (callback: (payload: T) => void) => () => void,
    handler: (payload: T) => void,
) {
    const onEvent = useEffectEvent(handler);

    useEffect(() => {
        return subscribe((payload) => onEvent(payload));
    }, [subscribe]);
}
