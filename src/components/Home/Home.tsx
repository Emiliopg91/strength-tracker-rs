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
import { useContext, useEffect, useEffectEvent, useState } from "react";
import "@/styles/Home/Home.css";
import "@/styles/Sessions/SessionModal.css";
import {
    SessionListItem,
    WorkoutDetails,
    WorkoutListItem,
} from "@/utils/backend/models";
import { BackendListener } from "@/utils/backend/listener";
import { SessionModal } from "../Sessions/modals/SessionModal";
import { WorkoutModal } from "../Workouts/modals/WorkoutModal";
import { Heatmap } from "./charts/Heatmap";
import { WorkloadChart } from "./charts/WorkloadChart";
import { WorkoutStepsModal } from "../Workouts/modals/WorkoutStepsModal";
import { WorkoutActionsMenu } from "../Workouts/menus/WorkoutActionsMenu";
import { Button } from "@mui/material";
import { ReportModal } from "../Report/modals/ReportModal";
import { ImportSessionsMenu } from "../Sessions/menus/ImportSessionsMenu";
import { differenceInCalendarDays } from "date-fns";

interface PeriodStats {
    sessions: number;
    time: number;
    kcal: number;
    load: number;
}

interface NextWorkout {
    /// Planned date (local midnight, seconds); undefined when it is just a suggestion
    date?: number;
    details: WorkoutListItem;
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
    const {
        translate,
        fromKg,
        formatDuration,
        formatTimeDate,
        formatWeekday,
        formatNumber,
    } = useContext(I18nSettingsContext);
    const adapter = usePickerAdapter();

    const day = useCurrentDay();
    const [availableData, setAvailableData] = useState(false);
    const [workload, setWorkload] = useState<WorkoutLoad[]>([]);
    const [stats, setStats] = useState<HomeStats>(emptyStats);
    const [heatMapData, setHeatMapData] = useState<
        [number, number][][] | undefined
    >(undefined);

    const [workout, setWorkout] = useState<NextWorkout | undefined>(undefined);
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
    const [reportModal, setReportModal] = useState(false);

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
                const newStats = calculateStats(sessions, today, adapter);

                withLoading(BackendClient.getHeatmapData()).then(
                    setHeatMapData,
                );

                Promise.all([
                    withLoading(BackendClient.getNextPlannedWorkout()),
                    withLoading(BackendClient.getWorkoutList()),
                ]).then(([next, workouts]) => {
                    const planned =
                        next !== null
                            ? workouts.find((w) => w.name == next.workout)
                            : undefined;

                    if (next !== null && planned) {
                        setRest(false);
                        setWorkout({ date: next.date, details: planned });
                        return;
                    }

                    // No planned workout: fall back to the activity suggestion
                    setRest(newStats.today.load > 50 || overreaching);

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
                        setWorkout({ details: oldest });
                    } else {
                        setWorkout(undefined);
                    }
                });

                setStats(newStats);

                setWorkload(workout_data);

                setLastSession(sessions[0]);
            }
        });
    };

    const isPlanned = workout?.date !== undefined;

    // "Today", the day of the week within a week, or how many days away the planned workout is
    const formatPlannedDay = (date: number) => {
        const diff = differenceInCalendarDays(date * 1000, new Date());
        if (diff <= 0) return translate("today");
        if (diff <= 7) return formatWeekday(date);
        return translate("in_n_days", [String(diff)]);
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
            prev && names.includes(prev.details.name)
                ? { ...prev, details: { ...prev.details, has_steps: true } }
                : prev,
        );
    });

    const onRefreshTrigger = useEffectEvent(refresh);

    useEffect(() => {
        onRefreshTrigger();
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
                            <div className="list-layer">
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
                                                        "subsport_" +
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
                            <legend>
                                {translate(
                                    isPlanned
                                        ? "next_planned_workout"
                                        : "activity_suggestion",
                                )}
                            </legend>
                            {!isPlanned && rest && (
                                <div style={{ textAlign: "center" }}>
                                    <p>{translate("rest_recommended")}</p>
                                </div>
                            )}
                            {workout && (isPlanned || !rest) && (
                                <div className="list-layer">
                                    <table>
                                        <thead>
                                            <tr>
                                                {isPlanned && (
                                                    <th className="text-center">
                                                        {translate("date")}
                                                    </th>
                                                )}
                                                <th className="text-center">
                                                    {translate("workout")}
                                                </th>
                                                <th className="text-center">
                                                    {translate(
                                                        "latest_session",
                                                    )}
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
                                                        workout.details.name,
                                                    )
                                                }
                                            >
                                                {workout.date !== undefined && (
                                                    <td className="text-center">
                                                        {formatPlannedDay(
                                                            workout.date,
                                                        )}
                                                    </td>
                                                )}
                                                <td className="text-center">
                                                    {workout.details.name
                                                        .length > 0 && (
                                                        <span>
                                                            {
                                                                workout.details
                                                                    .name
                                                            }
                                                        </span>
                                                    )}
                                                    {workout.details.name
                                                        .length == 0 && (
                                                        <span>
                                                            {translate("other")}
                                                        </span>
                                                    )}
                                                </td>
                                                <td>
                                                    {workout.details
                                                        .latest_session &&
                                                        formatTimeDate(
                                                            workout.details
                                                                .latest_session,
                                                        )}
                                                </td>
                                                <td>
                                                    {formatDuration(
                                                        workout.details
                                                            .avg_time,
                                                    )}
                                                </td>
                                                <td>
                                                    {workout.details.avg_load}
                                                </td>
                                                <td
                                                    onClick={(e) =>
                                                        e.stopPropagation()
                                                    }
                                                >
                                                    {workout.details
                                                        .has_steps && (
                                                        <WorkoutActionsMenu
                                                            workoutName={
                                                                workout.details
                                                                    .name
                                                            }
                                                            onOpenEdit={() =>
                                                                openEdit(
                                                                    workout
                                                                        .details
                                                                        .name,
                                                                )
                                                            }
                                                            send={true}
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

            <div>
                {reportModal && (
                    <ReportModal
                        onClose={() => {
                            setReportModal(false);
                        }}
                    />
                )}
            </div>

            <div
                style={{ display: "flex", gap: 5 }}
                className="list-action-bar equal-width-children"
            >
                <ImportSessionsMenu onImported={refresh} />
                <Button
                    variant="contained"
                    onClick={() => {
                        setReportModal(true);
                    }}
                >
                    {translate("generate_report")}
                </Button>
            </div>
        </>
    );
}
