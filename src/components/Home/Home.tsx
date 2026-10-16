import { AppContext } from "@/context/AppContext";
import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { useBackendEvent } from "@/hooks/useBackendEvent";
import { useCurrentDay } from "@/hooks/useCurrentDay";
import { useLoadingTask } from "@/hooks/useLoadingTask";
import { BackendClient } from "@/utils/backend/client";
import {
    SessionFrontDetails,
    SessionUtils,
    WorkoutLoad,
} from "@/utils/SessionUtils";
import { usePickerAdapter } from "@mui/x-date-pickers/hooks";
import { useContext, useEffect, useState } from "react";
import "@/styles/Home/Home.css";
import "@/styles/Sessions/SessionModal.css";
import {
    SessionListItem,
    WorkoutDetails,
    WorkoutListItem,
} from "@/utils/backend/models";
import { BackendListener } from "@/utils/backend/listener";
import { SessionModal } from "../Sessions/SessionModal";
import { WorkoutModal } from "../Workouts/WorkoutModal";
import { Heatmap } from "./helpers/Heatmap";
import { WorkloadChart } from "./helpers/WorkloadChart";
import { ImportSessionsMenu } from "./helpers/ImportSessionsMenu";
import { WorkoutStepsModal } from "../Workouts/WorkoutStepsModal";
import { WorkoutActionsMenu } from "../Workouts/helpers/WorkoutActionsMenu";

interface PeriodStats {
    sessions: number;
    time: number;
    kcal: number;
    load: number;
}

const PERIODS = ["today", "thisWeek", "previousWeek", "month"] as const;
type Period = (typeof PERIODS)[number];
type HomeStats = Record<Period, PeriodStats>;

const emptyPeriod = (): PeriodStats => ({
    sessions: 0,
    time: 0,
    kcal: 0,
    load: 0,
});

const emptyStats = (): HomeStats => ({
    today: emptyPeriod(),
    thisWeek: emptyPeriod(),
    previousWeek: emptyPeriod(),
    month: emptyPeriod(),
});

const addSession = (stats: PeriodStats, session: SessionListItem) => {
    stats.sessions += 1;
    stats.time += session.total_elapsed_time;
    stats.kcal += session.active_calories;
    stats.load += session.training_load;
};

// Sessions must be sorted by timestamp, most recent first
const calculateStats = (
    sessions: SessionListItem[],
    today: number,
    adapter: ReturnType<typeof usePickerAdapter>,
): HomeStats => {
    const stats = emptyStats();

    const startOfWeek = adapter.startOfWeek(adapter.startOfDay(new Date()));
    const thisWeekLimit = startOfWeek.getTime() / 1000;
    const prevWeekLimit = adapter.addWeeks(startOfWeek, -1).getTime() / 1000;
    const monthLimit = today - 29 * 24 * 60 * 60;

    for (const session of sessions) {
        if (session.timestamp < monthLimit) {
            break;
        }
        addSession(stats.month, session);
        if (session.timestamp >= thisWeekLimit) {
            addSession(stats.thisWeek, session);
            if (session.timestamp >= today) {
                addSession(stats.today, session);
            }
        } else if (session.timestamp >= prevWeekLimit) {
            addSession(stats.previousWeek, session);
        }
    }

    return stats;
};

export function Home() {
    const { sessionsVersion } = useContext(AppContext);
    const withLoading = useLoadingTask();
    const { translate, fromKg, formatDuration, formatTimeDate, formatNumber } =
        useContext(I18nSettingsContext);
    const adapter = usePickerAdapter();

    const day = useCurrentDay();
    const [availableData, setAvailableData] = useState(false);
    const [workload, setWorkload] = useState<WorkoutLoad[]>([]);
    const [stats, setStats] = useState<HomeStats>(emptyStats);
    const [heatMapData, setHeatMapData] = useState<
        [number, number][][] | undefined
    >(undefined);

    const [workout, setWorkout] = useState<WorkoutListItem | undefined>(
        undefined,
    );
    const [rest, setRest] = useState(false);
    const [workoutDetails, setWorkoutDetails] = useState<
        WorkoutDetails | undefined
    >(undefined);
    const [session, setLastSession] = useState<SessionListItem | undefined>(
        undefined,
    );
    const [sessionDetails, setSessionDetails] = useState<
        SessionFrontDetails | undefined
    >(undefined);
    const [workoutEdit, setWorkoutEdit] = useState<WorkoutDetails | undefined>(
        undefined,
    );

    const refresh = () => {
        const today =
            new Date(new Date().setHours(0, 0, 0, 0)).getTime() / 1000;
        withLoading(
            BackendClient.getSessions(
                today - SessionUtils.CHRONIC_DAYS * 2 * 24 * 60 * 60,
            ),
        ).then((sessions) => {
            setAvailableData(sessions.length > 0);

            if (sessions.length > 0) {
                const workout_data =
                    SessionUtils.calculateWorkoutLoad(sessions);
                const overreaching = SessionUtils.isOverreaching(workout_data);

                withLoading(BackendClient.getHeatmapData()).then(
                    setHeatMapData,
                );

                withLoading(BackendClient.getWorkoutList()).then((workouts) => {
                    const filtered = workouts.filter((w) => w.enabled);

                    if (!overreaching && filtered.length > 0) {
                        // Never-done workouts take priority, otherwise the least recent one
                        let oldest = filtered.find(
                            (w) => w.latest_session == null,
                        );
                        if (oldest === undefined) {
                            oldest = filtered[0];
                            for (const w of filtered) {
                                if (
                                    oldest.latest_session! > w.latest_session!
                                ) {
                                    oldest = w;
                                }
                            }
                        }
                        setWorkout(oldest);
                    } else {
                        setWorkout(undefined);
                    }
                });

                const newStats = calculateStats(sessions, today, adapter);
                setStats(newStats);

                setWorkload(workout_data);

                setLastSession(sessions[0]);

                setRest(newStats.today.load > 50 || overreaching);
            }
        });
    };

    const getSessionDetails = (timestamp: number) => {
        withLoading(BackendClient.getSessionDetails(timestamp)).then(
            (details) => {
                setSessionDetails(
                    SessionUtils.detailsFromBackend(details, fromKg),
                );
            },
        );
    };
    const getWorkoutDetails = (name: string) => {
        withLoading(BackendClient.getWorkoutDetails(name)).then(
            setWorkoutDetails,
        );
    };

    useBackendEvent(BackendListener.onSessionLocationUpdate, (data) => {
        setLastSession((prev) =>
            prev && prev.timestamp == data.session
                ? { ...prev, name: data.location }
                : prev,
        );
    });

    useBackendEvent(BackendListener.onAddedWorkoutSteps, (names) => {
        setWorkout((prev) =>
            prev && names.includes(prev.name)
                ? { ...prev, has_steps: true }
                : prev,
        );
    });

    useEffect(() => {
        refresh();
    }, [sessionsVersion, adapter, day]);

    const openEdit = (workout: string) => {
        withLoading(BackendClient.getWorkoutDetails(workout)).then(
            (details) => {
                setWorkoutDetails(undefined);
                setWorkoutEdit(details);
            },
        );
    };

    return (
        <>
            {!availableData && (
                <>
                    <h1 id="no-available-data">
                        {translate("no_available_data")}
                    </h1>
                    <h3 id="import-sessions">
                        {translate("import_session_to_begin")}
                    </h3>
                </>
            )}
            {availableData && (
                <>
                    <div className="dashboard-box">
                        <fieldset>
                            <legend>{translate("training_status")}</legend>
                            <div style={{ display: "flex" }}>
                                {workload.length > 0 && (
                                    <WorkloadChart
                                        workload={workload}
                                        minDate={workload[0].date}
                                    />
                                )}
                                <Heatmap data={heatMapData} />
                            </div>
                            <table id="last-days">
                                <colgroup>
                                    <col />
                                    <col />
                                    <col />
                                    <col />
                                </colgroup>
                                <thead>
                                    <tr>
                                        <th></th>
                                        <th>{translate("today")}</th>
                                        <th>{translate("this_week")}</th>
                                        <th>{translate("previous_week")}</th>
                                        <th>{translate("last_30_days")}</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    <tr>
                                        <td>{translate("sessions")}</td>
                                        {PERIODS.map((p) => (
                                            <td key={p}>
                                                {formatNumber(
                                                    stats[p].sessions,
                                                    0,
                                                )}
                                            </td>
                                        ))}
                                    </tr>
                                </tbody>
                                <tbody>
                                    <tr>
                                        <td>{translate("workout_load")}</td>
                                        {PERIODS.map((p) => (
                                            <td key={p}>
                                                {formatNumber(
                                                    stats[p].sessions > 0
                                                        ? stats[p].load /
                                                              stats[p].sessions
                                                        : 0,
                                                    0,
                                                ) +
                                                    " / " +
                                                    formatNumber(
                                                        stats[p].load,
                                                        0,
                                                    )}
                                            </td>
                                        ))}
                                    </tr>
                                </tbody>
                                <tbody>
                                    <tr>
                                        <td>{translate("active_time")}</td>
                                        {PERIODS.map((p) => (
                                            <td key={p}>
                                                {formatDuration(stats[p].time)}
                                            </td>
                                        ))}
                                    </tr>
                                </tbody>
                                <tbody>
                                    <tr>
                                        <td>{translate("active_calories")}</td>
                                        {PERIODS.map((p) => (
                                            <td key={p}>
                                                {formatNumber(stats[p].kcal, 0)}{" "}
                                                Kcal
                                            </td>
                                        ))}
                                    </tr>
                                </tbody>
                            </table>
                        </fieldset>
                    </div>
                    <div className="dashboard-box">
                        <fieldset>
                            <legend>{translate("last_session")}</legend>
                            <div id="list-layer">
                                <table>
                                    <thead>
                                        <tr>
                                            <th className="text-center">
                                                {translate("date")}
                                            </th>
                                            <th className="text-center">
                                                {translate("sport")}
                                            </th>
                                            <th className="text-center">
                                                {translate("name")}
                                            </th>
                                            <th className="text-center">
                                                {translate("active_calories")}
                                            </th>
                                            <th className="text-center">
                                                {translate("workout_load")}
                                            </th>
                                        </tr>
                                    </thead>

                                    <tbody>
                                        <tr
                                            className="clickable-row"
                                            onClick={() =>
                                                getSessionDetails(
                                                    session!.timestamp,
                                                )
                                            }
                                        >
                                            <td>
                                                {formatTimeDate(
                                                    session!.timestamp,
                                                )}
                                            </td>
                                            <td>
                                                {translate(
                                                    "sport_" + session!.sport,
                                                ) +
                                                    " - " +
                                                    translate(
                                                        "sport_" +
                                                            session!.sport +
                                                            "_" +
                                                            session!.sub_sport,
                                                    )}
                                            </td>
                                            <td>{session!.name}</td>
                                            <td>
                                                {formatNumber(
                                                    session!.active_calories,
                                                    0,
                                                )}
                                            </td>
                                            <td>
                                                {formatNumber(
                                                    session!.training_load,
                                                    0,
                                                )}
                                            </td>
                                        </tr>
                                    </tbody>
                                </table>
                            </div>
                        </fieldset>
                    </div>
                    <div className="dashboard-box">
                        <fieldset>
                            <legend>{translate("activity_suggestion")}</legend>
                            {rest && (
                                <div style={{ textAlign: "center" }}>
                                    <p>{translate("rest_recommended")}</p>
                                </div>
                            )}
                            {!rest && workout && (
                                <div id="list-layer">
                                    <table>
                                        <thead>
                                            <tr>
                                                <th className="text-center">
                                                    {translate("workout")}
                                                </th>
                                                <th className="text-center">
                                                    {translate(
                                                        "latest_session",
                                                    )}
                                                </th>
                                                <th className="text-center">
                                                    {translate("session_count")}
                                                </th>
                                                <th className="text-center">
                                                    {translate(
                                                        "average_duration",
                                                    )}
                                                </th>
                                                <th className="text-center">
                                                    {translate("avg_workload")}
                                                </th>
                                                <th></th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            <tr
                                                className="clickable-row"
                                                onClick={() =>
                                                    getWorkoutDetails(
                                                        workout.name,
                                                    )
                                                }
                                            >
                                                <td className="text-left">
                                                    {workout.name.length >
                                                        0 && (
                                                        <span>
                                                            {workout.name}
                                                        </span>
                                                    )}
                                                    {workout.name.length ==
                                                        0 && (
                                                        <span>
                                                            {translate("other")}
                                                        </span>
                                                    )}
                                                </td>
                                                <td>
                                                    {workout.latest_session &&
                                                        formatTimeDate(
                                                            workout.latest_session,
                                                        )}
                                                </td>
                                                <td>
                                                    {formatNumber(
                                                        workout.sessions,
                                                        0,
                                                    )}
                                                </td>
                                                <td>
                                                    {formatDuration(
                                                        workout.avg_time,
                                                    )}
                                                </td>
                                                <td>{workout.avg_load}</td>
                                                <td
                                                    onClick={(e) =>
                                                        e.stopPropagation()
                                                    }
                                                >
                                                    {workout.has_steps && (
                                                        <WorkoutActionsMenu
                                                            workoutName={
                                                                workout.name
                                                            }
                                                            onOpenEdit={() =>
                                                                openEdit(
                                                                    workout.name,
                                                                )
                                                            }
                                                            dense
                                                        />
                                                    )}
                                                </td>
                                            </tr>
                                        </tbody>
                                    </table>
                                </div>
                            )}
                        </fieldset>
                    </div>
                </>
            )}

            <div>
                {sessionDetails && (
                    <SessionModal
                        session={sessionDetails}
                        onClose={() => setSessionDetails(undefined)}
                        onUpdate={() => {
                            refresh();
                        }}
                    />
                )}
            </div>

            <div>
                {workoutDetails && (
                    <WorkoutModal
                        workout={workoutDetails}
                        showEnable={false}
                        onClose={() => setWorkoutDetails(undefined)}
                        onOpenEdit={() => {
                            openEdit(workoutDetails.name);
                        }}
                    />
                )}
            </div>

            <div>
                {workoutEdit && (
                    <WorkoutStepsModal
                        workout={workoutEdit}
                        isEdit={true}
                        onClose={() => setWorkoutEdit(undefined)}
                    />
                )}
            </div>

            <ImportSessionsMenu onImported={refresh} />
        </>
    );
}
