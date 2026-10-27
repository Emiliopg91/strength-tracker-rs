import { WorkoutDetails, WorkoutListItem } from "@/utils/backend/models";
import { WorkoutModal } from "./WorkoutModal";
import { WorkoutRow } from "./helpers/WorkoutRow";
import { BackendClient } from "@/utils/backend/client";
import { useContext, useState } from "react";
import { AppContext } from "@/context/AppContext";
import { useBackendEvent } from "@/hooks/useBackendEvent";
import { useBackendList } from "@/hooks/useBackendList";
import { useLoadingTask } from "@/hooks/useLoadingTask";
import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import "@/styles/Workouts/WorkoutLists.css";
import { WorkoutStepsModal } from "./WorkoutStepsModal";
import { Button } from "@mui/material";
import { WorkoutUtils } from "@/utils/WorkoutUtils";
import { BackendListener } from "@/utils/backend/listener";
import { useFilterList } from "@/hooks/useFilterList";

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

    const openEdit = (workout: WorkoutDetails | undefined, edit: boolean) => {
        setIsEdit(edit);
        if (edit) {
            setWorkoutEdit(workout);
        } else {
            if (workout === undefined) {
                const workout: WorkoutDetails = {
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
                        "",
                        toKg,
                    ),
                    primary_muscles: [],
                    secondary_muscles: [],
                };
                setWorkoutEdit(workout);
            } else {
                workout.name = workout.name + " - 2";
                setWorkoutEdit(workout);
            }
        }
        setWorkoutDetails(undefined);
    };

    useBackendEvent(BackendListener.onAddedWorkoutSteps, (names) => {
        setWorkouts((prev) =>
            prev.map((w) =>
                names.includes(w.name) ? { ...w, has_steps: true } : w,
            ),
        );
    });

    const getWorkoutDetails = (name: string) => {
        withLoading(BackendClient.getWorkoutDetails(name)).then((details) => {
            setWorkoutDetails(details);
        });
    };

    const openEditByName = (name: string) => {
        withLoading(BackendClient.getWorkoutDetails(name)).then((workout) =>
            openEdit(workout, true),
        );
    };

    const openDuplicateByName = (name: string) => {
        withLoading(BackendClient.getWorkoutDetails(name)).then((workout) => {
            openEdit(workout, false);
        });
    };

    const setWorkoutState = (name: string, status: boolean) => {
        const res = workouts.map((w) => {
            if (w.name == name) {
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

    const changeEnabled = (workout: string, enabled: boolean) => {
        setWorkouts((prev) =>
            sortWorkouts(
                prev.map((wk) => {
                    if (wk.name == workout) {
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
            <div id="list-layer">
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
                                key={workout.name}
                                workout={workout}
                                onSelect={getWorkoutDetails}
                                onOpenEdit={openEditByName}
                                onOpenDuplicate={openDuplicateByName}
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
                            changeEnabled(workoutDetails.name, enabled)
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

            <div className="list-action-bar">
                <Button
                    className="full-width-button"
                    onClick={() => openEdit(undefined, false)}
                    variant="contained"
                >
                    {translate("create_workout")}
                </Button>
            </div>
        </>
    );
}
