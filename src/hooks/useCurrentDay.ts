import { useEffect, useState } from "react";

// Current date as a string, updated when the day changes
export function useCurrentDay() {
    const [day, setDay] = useState(() => new Date().toDateString());

    useEffect(() => {
        const checkDay = () => setDay(new Date().toDateString());

        const now = new Date();
        const nextMidnight = new Date(
            now.getFullYear(),
            now.getMonth(),
            now.getDate() + 1,
        );
        const timer = setTimeout(
            checkDay,
            nextMidnight.getTime() - now.getTime() + 1000,
        );

        // Timers may not fire on time after the system suspends
        document.addEventListener("visibilitychange", checkDay);
        window.addEventListener("focus", checkDay);

        return () => {
            clearTimeout(timer);
            document.removeEventListener("visibilitychange", checkDay);
            window.removeEventListener("focus", checkDay);
        };
    }, [day]);

    return day;
}
