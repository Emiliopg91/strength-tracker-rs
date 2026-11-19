import { useContext, useMemo, useRef } from "react";
import { Button, IconButton } from "@mui/material";
import { DateCalendar } from "@mui/x-date-pickers/DateCalendar";
import DeleteIcon from "@mui/icons-material/Delete";
import EventIcon from "@mui/icons-material/Event";
import { addMonths, format } from "date-fns";
import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { useCurrentDay } from "@/hooks/useCurrentDay";
import { WorkoutPlanListItem } from "@/utils/backend/models";
import "@/styles/Workouts/Planning.css";

// Local calendar day key, independent of the time of day
const dayKey = (date: Date) => format(date, "yyyy-MM-dd");

// Minimum time between two month changes, so a trackpad's burst of wheel events moves one month
const WHEEL_MONTH_THROTTLE_MS = 250;

// MUI's month header and its arrows are left out: a smaller title replaces it and the month
// changes with the mouse wheel
const NoCalendarHeader = () => null;

// A plan is done once a session of its workout was imported on its day
const isDone = (plan: WorkoutPlanListItem) => plan.session != null;

type Props = {
    plans: WorkoutPlanListItem[];
    selectedDay: Date;
    onSelectDay: (day: Date) => void;
    onRemove: (plan: WorkoutPlanListItem) => void;
    /** Opens the workout picker for the selected day. */
    onPlan: () => void;
    canPlan: boolean;
};

/** Planning calendar with the workouts planned for the selected day beside it. */
export function PlanningPanel({
    plans,
    selectedDay,
    onSelectDay,
    onRemove,
    onPlan,
    canPlan,
}: Props) {
    const { translate, formatDate, formatWeekday, formatMonth } =
        useContext(I18nSettingsContext);

    // Re-renders at midnight so the past-day check stays current
    const currentDay = useCurrentDay();
    // yyyy-MM-dd keys compare chronologically as strings
    const today = dayKey(new Date(currentDay));

    const byDay: Record<string, WorkoutPlanListItem[]> = useMemo(() => {
        const res: Record<string, WorkoutPlanListItem[]> = {};
        plans.forEach((plan) => {
            const key = dayKey(new Date(plan.date * 1000));
            if (res[key] === undefined) {
                res[key] = [];
            }
            res[key].push(plan);
        });

        return res;
    }, [plans]);

    const selectedPlans = byDay[dayKey(selectedDay)] ?? [];

    // Days are green once every plan was done, whatever the day, and past days red otherwise
    const plannedDayClasses = useMemo(() => {
        const res = new Map<string, string>();
        Object.entries(byDay).forEach(([day, list]) => {
            if (list.length == 0) return;
            if (list.every(isDone)) {
                res.set(day, "planned-day green");
            } else if (day >= today) {
                res.set(day, "planned-day");
            } else {
                res.set(day, "planned-day red");
            }
        });
        return res;
    }, [byDay, today]);

    // Wheel down moves to the next month, keeping the day number where the month has it
    const lastWheel = useRef(0);
    const onWheel = (event: React.WheelEvent) => {
        if (
            event.deltaY == 0 ||
            event.timeStamp - lastWheel.current < WHEEL_MONTH_THROTTLE_MS
        ) {
            return;
        }
        lastWheel.current = event.timeStamp;
        onSelectDay(addMonths(selectedDay, event.deltaY > 0 ? 1 : -1));
    };

    const isPast = (plan: WorkoutPlanListItem) =>
        dayKey(new Date(plan.date * 1000)) < today;

    const dotClass = (plan: WorkoutPlanListItem) => {
        if (isDone(plan)) return "green";
        return isPast(plan) ? "red" : "";
    };

    return (
        <div className="planning-layout planning-panel">
            <div className="planning-calendar" onWheel={onWheel}>
                <div className="planning-calendar-month">
                    {formatMonth(selectedDay.getTime() / 1000)}
                </div>
                <DateCalendar
                    value={selectedDay}
                    onChange={(value) => {
                        if (value != null) onSelectDay(value);
                    }}
                    showDaysOutsideCurrentMonth
                    fixedWeekNumber={6}
                    slots={{ calendarHeader: NoCalendarHeader }}
                    slotProps={{
                        day: (ownerState) => ({
                            className: plannedDayClasses.get(
                                dayKey(ownerState.day),
                            ),
                        }),
                    }}
                />
            </div>

            <div className="planning-day">
                <div className="planning-day-title">
                    {formatWeekday(selectedDay.getTime() / 1000)},{" "}
                    {formatDate(selectedDay.getTime() / 1000)}
                </div>
                {selectedPlans.length == 0 ? (
                    <div className="planning-empty">
                        <EventIcon fontSize="large" />
                        <span>{translate("no_planned_workouts")}</span>
                    </div>
                ) : (
                    <ul className="planning-list">
                        {selectedPlans.map((plan) => (
                            <li
                                key={`${plan.date}-${plan.workout}`}
                                className="planning-item"
                            >
                                <span
                                    className={`planning-item-dot ${dotClass(plan)}`}
                                />
                                <span className="planning-item-name">
                                    {plan.workout}
                                </span>
                                {!isPast(plan) && !isDone(plan) && (
                                    <IconButton
                                        size="small"
                                        title={translate("delete")}
                                        onClick={() => onRemove(plan)}
                                    >
                                        <DeleteIcon fontSize="small" />
                                    </IconButton>
                                )}
                            </li>
                        ))}
                    </ul>
                )}
                <Button
                    variant="contained"
                    className="full-width-button planning-day-action"
                    disabled={!canPlan}
                    onClick={onPlan}
                >
                    {translate("plan_workout")}
                </Button>
            </div>
        </div>
    );
}
