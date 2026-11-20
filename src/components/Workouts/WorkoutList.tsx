import { WorkoutDetails, WorkoutListItem } from "@/utils/backend/models";
import { WorkoutModal } from "./modals/WorkoutModal";
import { WorkoutRow } from "./rows/WorkoutRow";
import { BackendClient } from "@/utils/backend/client";
import { useContext, useState } from "react";
import { AppContext } from "@/context/AppContext";
import { useBackendEvent } from "@/hooks/useBackendEvent";
import { useBackendList } from "@/hooks/useBackendList";
import { useLoadingTask } from "@/hooks/useLoadingTask";
import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import "@/styles/Workouts/WorkoutLists.css";
import { WorkoutStepsModal } from "./modals/WorkoutStepsModal";
import { Button } from "@mui/material";
import { WorkoutUtils } from "@/utils/WorkoutUtils";
import { BackendListener } from "@/utils/backend/listener";
import { useFilterList } from "@/hooks/useFilterList";
import { PlanningPanel } from "./PlanningPanel";
import { PlanWorkoutModal } from "./modals/PlanWorkoutModal";
import { WorkoutPlanListItem } from "@/utils/backend/models";
import { useCurrentDay } from "@/hooks/useCurrentDay";
import { isSameDay, startOfDay } from "date-fns";

export function WorkoutsList() {
    const { sessionsVersion } = useContext(AppContext);
    const { translate, toKg, formatTimeDate } = useContext(I18nSettingsContext);
    const withLoading = useLoadingTask();

    const sortWorkouts = (workouts: WorkoutListItem[]): WorkoutListItem[] => {
        return workouts.sort((a, b) => {
            if (a.enabled && !b.enabled) {
                return -1;
            } else if (b.enabled && !a.enabled) {
                return 1;
            } else if (a.name.length > 0 && b.name.length > 0) {
                return a.name.localeCompare(b.name);
            } else {
                if (a.name.length == 0) {
                    return 1;
                } else {
                    return -1;
                }
            }
        });
    };

    const [workouts, setWorkouts, refreshList] =
        useBackendList<WorkoutListItem>(
            () =>
                BackendClient.getWorkoutList().then((data) =>
                    sortWorkouts(data),
                ),
            [sessionsVersion],
        );
    const [workoutDetails, setWorkoutDetails] = useState<
        WorkoutDetails | undefined
    >(undefined);
    const [workoutEdit, setWorkoutEdit] = useState<WorkoutDetails | undefined>(
        undefined,
    );
    const [isEdit, setIsEdit] = useState(false);
    const [planning, setPlanning] = useState(false);
    const [selectedDay, setSelectedDay] = useState(() =>
        startOfDay(new Date()),
    );
    // Re-renders at midnight so the past-day check stays current
    const currentDay = useCurrentDay();
    const [plans, , refreshPlans] = useBackendList(
        BackendClient.getPlannedWorkouts,
        [sessionsVersion],
    );

    const isPastDay = selectedDay < startOfDay(new Date(currentDay));
    const availableWorkouts = workouts.filter(
        (w) =>
            w.enabled &&
            !plans.some(
                (p) =>
                    p.workout == w.id &&
                    isSameDay(new Date(p.date * 1000), selectedDay),
            ),
    );
    const workoutNames = new Map(workouts.map((w) => [w.id, w.name]));

    const addPlan = (workout: number) => {
        withLoading(
            BackendClient.addPlannedWorkout(
                selectedDay.getTime() / 1000,
                workout,
            ),
        ).then(() => {
            setPlanning(false);
            refreshPlans();
        });
    };

    const removePlan = (plan: WorkoutPlanListItem) => {
        withLoading(
            BackendClient.deletePlannedWorkout(plan.date, plan.workout),
        ).then(refreshPlans);
    };

    const openEdit = (workout: WorkoutDetails | undefined, edit: boolean) => {
        setIsEdit(edit);
        if (edit) {
            setWorkoutEdit(workout);
        } else {
            if (workout === undefined) {
                const workout: WorkoutDetails = {
                    id: 0,
                    name: "",
                    avg_time: 0,
                    avg_volume: 0,
                    enabled: true,
                    latest_session: 0,
                    session_count: 0,
                    sessions: [],
                    notes: null,
                    steps: WorkoutUtils.toWorkoutSteps(
                        [structuredClone(WorkoutUtils.DEFAULT_GROUP)],
                        0,
                        toKg,
                    ),
                    primary_muscles: [],
                    secondary_muscles: [],
                };
                setWorkoutEdit(workout);
            } else {
                // A duplicate is a new workout, so it gets an id of its own when saved
                setWorkoutEdit({
                    ...workout,
                    id: 0,
                    name: workout.name + " - 2",
                });
            }
        }
        setWorkoutDetails(undefined);
    };

    useBackendEvent(BackendListener.onAddedWorkoutSteps, (ids) => {
        setWorkouts((prev) =>
            prev.map((w) =>
                ids.includes(w.id) ? { ...w, has_steps: true } : w,
            ),
        );
    });

    const getWorkoutDetails = (id: number) => {
        withLoading(BackendClient.getWorkoutDetails(id)).then((details) => {
            setWorkoutDetails(details);
        });
    };

    const openEditById = (id: number) => {
        withLoading(BackendClient.getWorkoutDetails(id)).then((workout) =>
            openEdit(workout, true),
        );
    };

    const openDuplicateById = (id: number) => {
        withLoading(BackendClient.getWorkoutDetails(id)).then((workout) => {
            openEdit(workout, false);
        });
    };

    const setWorkoutState = (id: number, status: boolean) => {
        const res = workouts.map((w) => {
            if (w.id == id) {
                return { ...w, enabled: status };
            } else {
                return { ...w };
            }
        });
        setWorkouts(res);
    };

    const [filter, filteredWorkouts] = useFilterList(workouts, (workout) => {
        if (workout.latest_session !== null) {
            return [workout.name, formatTimeDate(workout.latest_session)];
        } else {
            return [workout.name];
        }
    });

    const changeEnabled = (workout: number, enabled: boolean) => {
        setWorkouts((prev) =>
            sortWorkouts(
                prev.map((wk) => {
                    if (wk.id == workout) {
                        wk.enabled = enabled;
                    }
                    return wk;
                }),
            ),
        );
    };

    return (
        <>
            {filter}
            <div className="list-layer">
                <table>
                    <thead>
                        <tr>
                            <th className="text-center">
                                {translate("workout")}
                            </th>
                            <th className="text-center">
                                {translate("latest_session")}
                            </th>
                            <th className="text-center">
                                {translate("session_count")}
                            </th>
                            <th className="text-center">
                                {translate("average_duration")}
                            </th>
                            <th className="text-center">
                                {translate("avg_workload")}
                            </th>
                            <th></th>
                        </tr>
                    </thead>
                    <tbody>
                        {filteredWorkouts.map((workout) => (
                            <WorkoutRow
                                key={workout.id}
                                workout={workout}
                                onSelect={getWorkoutDetails}
                                onOpenEdit={openEditById}
                                onOpenDuplicate={openDuplicateById}
                            />
                        ))}
                    </tbody>
                </table>
            </div>

            <div>
                {workoutDetails && (
                    <WorkoutModal
                        workout={workoutDetails}
                        showEnable={true}
                        onClose={() => setWorkoutDetails(undefined)}
                        onUpdate={setWorkoutState}
                        onOpenEdit={(workout) => openEdit(workout, true)}
                        onEnabledChange={(enabled) =>
                            changeEnabled(workoutDetails.id, enabled)
                        }
                    />
                )}
            </div>

            <div>
                {workoutEdit && (
                    <WorkoutStepsModal
                        workout={workoutEdit}
                        isEdit={isEdit}
                        onClose={() => {
                            setWorkoutEdit(undefined);
                            refreshList();
                        }}
                    />
                )}
            </div>

            {planning && (
                <PlanWorkoutModal
                    day={selectedDay}
                    workouts={availableWorkouts}
                    onAdd={addPlan}
                    onClose={() => setPlanning(false)}
                />
            )}

            <div
                style={{ display: "flex", gap: 5 }}
                className="list-action-bar equal-width-children"
            >
                <Button
                    className="full-width-button"
                    onClick={() => openEdit(undefined, false)}
                    variant="contained"
                >
                    {translate("create_workout")}
                </Button>
            </div>

            <PlanningPanel
                plans={plans}
                workoutNames={workoutNames}
                selectedDay={selectedDay}
                onSelectDay={setSelectedDay}
                onRemove={removePlan}
                onPlan={() => setPlanning(true)}
                canPlan={!isPastDay && availableWorkouts.length > 0}
            />
        </>
    );
}
