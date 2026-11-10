import { JSX, useCallback, useState } from "react";

/// Rows mounted at first and on each scroll near the end of the list.
const ROWS_BATCH = 50;

/// How far ahead of the end of the list the next batch starts mounting.
const PRELOAD_MARGIN = "600px";

/// Closest ancestor that scrolls vertically, `null` (the viewport) if none.
function scrollParent(element: HTMLElement): HTMLElement | null {
    for (let e = element.parentElement; e; e = e.parentElement) {
        const { overflowY } = getComputedStyle(e);
        if (overflowY === "auto" || overflowY === "scroll") {
            return e;
        }
    }
    return null;
}

/// Mounting long tables at once takes hundreds of ms, so their rows are added in batches as the end
/// of the list scrolls into view. Returns the rows to render and the sentinel row to place after
/// them inside the `<tbody>` (`null` once every row is mounted).
export function useBatchedRows<T>(items: T[]): [T[], JSX.Element | null] {
    const [visibleRows, setVisibleRows] = useState(ROWS_BATCH);

    const observeSentinel = useCallback((row: HTMLTableRowElement | null) => {
        if (!row) return;
        // The scroll container clips the sentinel, so the margin only applies with it as root
        const observer = new IntersectionObserver(
            ([entry]) => {
                if (entry.isIntersecting) {
                    setVisibleRows((rows) => rows + ROWS_BATCH);
                }
            },
            { root: scrollParent(row), rootMargin: PRELOAD_MARGIN },
        );
        observer.observe(row);
        return () => observer.disconnect();
    }, []);

    const sentinel =
        visibleRows < items.length ? (
            // Remounted on every batch so the observer checks again whether it's still in view
            <tr key={visibleRows} ref={observeSentinel} />
        ) : null;

    return [items.slice(0, visibleRows), sentinel];
}
