import { useContext, useMemo } from "react";
import { Button, IconButton } from "@mui/material";
import { DateCalendar } from "@mui/x-date-pickers/DateCalendar";
import DeleteIcon from "@mui/icons-material/Delete";
import EventIcon from "@mui/icons-material/Event";
import { format } from "date-fns";
import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { useCurrentDay } from "@/hooks/useCurrentDay";
import { WorkoutPlanListItem } from "@/utils/backend/models";
import "@/styles/Workouts/Planning.css";

// Local calendar day key, independent of the time of day
const dayKey = (date: Date) => format(date, "yyyy-MM-dd");

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
    const { translate, formatDate, formatWeekday } =
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

    // Past days are green when every plan was done and red otherwise
    const plannedDayClasses = useMemo(() => {
        const res = new Map<string, string>();
        Object.entries(byDay).forEach(([day, list]) => {
            if (list.length == 0) return;
            if (day >= today) {
                res.set(day, "planned-day");
            } else if (list.every((wp) => wp.session != null)) {
                res.set(day, "planned-day green");
            } else {
                res.set(day, "planned-day red");
            }
        });
        return res;
    }, [byDay, today]);

    const isPast = (plan: WorkoutPlanListItem) =>
        dayKey(new Date(plan.date * 1000)) < today;

    const dotClass = (plan: WorkoutPlanListItem) => {
        if (!isPast(plan)) return "";
        return plan.session != null ? "green" : "red";
    };

    return (
        <div className="planning-layout planning-panel">
            <div className="planning-calendar">
                <DateCalendar
                    value={selectedDay}
                    onChange={(value) => {
                        if (value != null) onSelectDay(value);
                    }}
                    showDaysOutsideCurrentMonth
                    fixedWeekNumber={6}
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
                                {!isPast(plan) && (
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
