import { useContext, useMemo, useState } from "react";
import {
    Button,
    Dialog,
    DialogContent,
    DialogTitle,
    IconButton,
} from "@mui/material";
import CloseIcon from "@mui/icons-material/Close";
import { DateCalendar } from "@mui/x-date-pickers/DateCalendar";
import DeleteIcon from "@mui/icons-material/Delete";
import EventIcon from "@mui/icons-material/Event";
import { format, startOfDay } from "date-fns";
import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { useBackendList } from "@/hooks/useBackendList";
import { useCurrentDay } from "@/hooks/useCurrentDay";
import { BackendClient } from "@/utils/backend/client";
import { PlanWorkoutModal } from "./PlanWorkoutModal";
import "@/styles/Workouts/Planning.css";
import { useLoadingTask } from "@/hooks/useLoadingTask";
import { WorkoutPlanListItem } from "@/utils/backend/models";

// Local calendar day key, independent of the time of day
const dayKey = (date: Date) => format(date, "yyyy-MM-dd");

type Props = {
    onClose: () => void;
};

export function PlanningModal({ onClose }: Props) {
    const { translate } = useContext(I18nSettingsContext);

    // Re-renders at midnight so the past-day check stays current
    const currentDay = useCurrentDay();
    const [workouts] = useBackendList(BackendClient.getWorkoutList, []);
    const [selectedDay, setSelectedDay] = useState(() =>
        startOfDay(new Date()),
    );
    const [planning, setPlanning] = useState(false);
    const loadingTask = useLoadingTask();

    const [items, , refresh] = useBackendList(
        BackendClient.getPlannedWorkouts,
        [],
    );

    const plans: Record<string, WorkoutPlanListItem[]> = useMemo(() => {
        const res: Record<string, WorkoutPlanListItem[]> = {};
        items.forEach((item) => {
            const key = dayKey(new Date(item.date * 1000));
            if (res[key] === undefined) {
                res[key] = [];
            }
            res[key].push(item);
        });

        return res;
    }, [items]);

    const selectedKey = dayKey(selectedDay);
    // yyyy-MM-dd keys compare chronologically as strings
    const selectedPlans = plans[selectedKey] ?? [];

    // Past days are green when every plan was done and red otherwise
    const plannedDayClasses = useMemo(() => {
        const today = dayKey(new Date(currentDay));
        const res = new Map<string, string>();
        Object.entries(plans).forEach(([day, list]) => {
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
    }, [plans, currentDay]);

    const availableWorkouts = workouts
        .filter(
            (w) =>
                w.enabled && !selectedPlans.find((wp) => wp.workout == w.name),
        )
        .map((w) => w.name);

    const addPlan = (workout: string) => {
        loadingTask(
            BackendClient.addPlannedWorkout(
                selectedDay.getTime() / 1000,
                workout,
            ),
        ).then(() => {
            setPlanning(false);
            refresh();
        });
    };

    const removePlan = (workout: WorkoutPlanListItem) => {
        loadingTask(
            BackendClient.deletePlannedWorkout(workout.date, workout.workout),
        ).then(refresh);
    };

    const isPastWorkout = (workout: WorkoutPlanListItem) => {
        return (
            dayKey(new Date(workout.date * 1000)) < dayKey(new Date(currentDay))
        );
    };
    const isPastDay = selectedKey < dayKey(new Date(currentDay));

    return (
        <Dialog open={true} onClose={onClose} maxWidth="md" fullWidth>
            <DialogTitle>
                {translate("planning")}
                <IconButton onClick={onClose} className="modal-close-button">
                    <CloseIcon />
                </IconButton>
            </DialogTitle>

            <DialogContent dividers>
                <div className="planning-layout">
                    <div className="planning-calendar">
                        <DateCalendar
                            value={selectedDay}
                            onChange={(value) => {
                                if (value != null) setSelectedDay(value);
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
                        {selectedPlans.length == 0 ? (
                            <div className="planning-empty">
                                <EventIcon fontSize="large" />
                                <span>{translate("no_planned_workouts")}</span>
                            </div>
                        ) : (
                            <ul className="planning-list">
                                {selectedPlans.map((workout) => (
                                    <li
                                        key={`${workout.date}-${workout.workout}`}
                                        className="planning-item"
                                    >
                                        {isPastWorkout(workout) &&
                                            workout.session != null && (
                                                <span className="planning-item-dot green" />
                                            )}
                                        {isPastWorkout(workout) &&
                                            workout.session == null && (
                                                <span className="planning-item-dot red" />
                                            )}
                                        {!isPastWorkout(workout) && (
                                            <span className="planning-item-dot" />
                                        )}
                                        <span className="planning-item-name">
                                            {workout.workout}
                                        </span>
                                        {!isPastWorkout(workout) && (
                                            <IconButton
                                                size="small"
                                                title={translate("delete")}
                                                onClick={() =>
                                                    removePlan(workout)
                                                }
                                            >
                                                <DeleteIcon fontSize="small" />
                                            </IconButton>
                                        )}
                                    </li>
                                ))}
                            </ul>
                        )}
                    </div>
                </div>

                <hr />
                <Button
                    variant="contained"
                    className="full-width-button"
                    disabled={isPastDay || availableWorkouts.length == 0}
                    onClick={() => setPlanning(true)}
                >
                    {translate("plan_workout")}
                </Button>
            </DialogContent>

            {planning && (
                <PlanWorkoutModal
                    day={selectedDay}
                    workouts={availableWorkouts}
                    onAdd={addPlan}
                    onClose={() => setPlanning(false)}
                />
            )}
        </Dialog>
    );
}
