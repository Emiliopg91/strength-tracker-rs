import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { WorkoutDetails } from "@/utils/backend/models";
import { useContext, useEffect, useState } from "react";
import {
    Box,
    Dialog,
    DialogContent,
    DialogTitle,
    IconButton,
    Switch,
} from "@mui/material";
import CloseIcon from "@mui/icons-material/Close";
import { BackendClient } from "@/utils/backend/client";
import { useLoadingTask } from "@/hooks/useLoadingTask";
import { WorkoutVolumeChart } from "./helpers/WorkoutVolumeChart";
import { WorkoutActionsMenu } from "./helpers/WorkoutActionsMenu";
import { BackendListener } from "@/utils/backend/listener";

type Props = {
    workout: WorkoutDetails;
    showEnable: boolean;
    onClose: () => void;
    onUpdate?: (name: string, status: boolean) => void;
    onOpenEdit: (workout: WorkoutDetails) => void;
    onOpenDuplicate?: (workout: WorkoutDetails) => void;
};

export function WorkoutModal({
    workout: initialWorkout,
    showEnable,
    onClose,
    onUpdate,
    onOpenEdit,
    onOpenDuplicate,
}: Props) {
    const withLoading = useLoadingTask();
    const {
        translate,
        fromKg,
        getWeightUnit,
        formatDuration,
        formatTimeDate,
        formatNumber,
        formatPercent,
    } = useContext(I18nSettingsContext);
    const [workout, setWorkout] = useState(initialWorkout);
    const [enabled, setEnabled] = useState(initialWorkout.suggest);

    // Reset to the new prop during render instead of in an effect
    const [prevInitialWorkout, setPrevInitialWorkout] =
        useState(initialWorkout);
    if (prevInitialWorkout !== initialWorkout) {
        setPrevInitialWorkout(initialWorkout);
        setWorkout(initialWorkout);
    }

    useEffect(() => {
        return BackendListener.onAddedWorkoutSteps((names) => {
            if (names.includes(initialWorkout.name)) {
                BackendClient.getWorkoutDetails(initialWorkout.name).then(
                    setWorkout,
                );
            }
        });
    }, [initialWorkout.name]);

    const toggleEnabled = () => {
        withLoading(
            BackendClient.setWorkoutStatus(workout.name, !enabled),
        ).then(() => {
            if (onUpdate) {
                onUpdate(workout.name, !enabled);
            }
            setEnabled((prev) => !prev);
        });
    };

    return (
        <Dialog open={true} onClose={onClose} fullWidth maxWidth="md">
            <DialogTitle>
                {workout.name.length > 0 && <span>{workout.name}</span>}
                {workout.name.length == 0 && <span>{translate("other")}</span>}
                {workout.name.length > 0 && workout.steps.length > 0 && (
                    // Positioned like .modal-close-button, just to its left
                    <Box
                        sx={{ position: "absolute", right: "48px", top: "8px" }}
                    >
                        {onOpenDuplicate && (
                            <WorkoutActionsMenu
                                workoutName={workout.name}
                                onOpenEdit={() => onOpenEdit(workout)}
                                onOpenDuplicate={() => onOpenDuplicate(workout)}
                            />
                        )}
                        {!onOpenDuplicate && (
                            <WorkoutActionsMenu
                                workoutName={workout.name}
                                onOpenEdit={() => onOpenEdit(workout)}
                            />
                        )}
                    </Box>
                )}
                <IconButton onClick={onClose} className="modal-close-button">
                    <CloseIcon />
                </IconButton>
            </DialogTitle>

            <DialogContent dividers>
                <div style={{ display: "flex" }}>
                    <div style={{ flex: 1 }}>
                        <table id="workout-details-table">
                            <colgroup>
                                <col className="col-200" />
                                <col className="col-150" />
                                <col />
                            </colgroup>
                            <tbody>
                                {workout.session_count > 0 && (
                                    <>
                                        <tr>
                                            <td>{translate("sessions")}:</td>
                                            <td>
                                                {formatNumber(
                                                    workout.session_count,
                                                    0,
                                                )}
                                            </td>
                                        </tr>
                                        <tr>
                                            <td>
                                                {translate("latest_session")}
                                            </td>
                                            <td>
                                                {workout.latest_session &&
                                                    formatTimeDate(
                                                        workout.latest_session,
                                                    )}
                                            </td>
                                        </tr>
                                        <tr>
                                            <td>{translate("average_time")}</td>
                                            <td>
                                                {formatDuration(
                                                    workout.avg_time,
                                                )}
                                            </td>
                                        </tr>
                                        {workout.name.length > 0 && (
                                            <tr>
                                                <td>
                                                    {translate(
                                                        "average_volume",
                                                    )}
                                                    :
                                                </td>
                                                <td>
                                                    {formatNumber(
                                                        fromKg(
                                                            workout.avg_volume,
                                                        ),
                                                        1,
                                                    )}{" "}
                                                    {getWeightUnit()}
                                                </td>
                                            </tr>
                                        )}
                                    </>
                                )}
                                {showEnable && (
                                    <tr>
                                        <td>
                                            {translate("suggestions_enabled")}:
                                        </td>
                                        <td>
                                            <Switch
                                                checked={enabled}
                                                onChange={toggleEnabled}
                                                slotProps={{
                                                    input: {
                                                        "aria-label": translate(
                                                            "suggestions_enabled",
                                                        ),
                                                    },
                                                }}
                                            />
                                        </td>
                                    </tr>
                                )}
                            </tbody>
                        </table>
                    </div>
                </div>
                {workout.sessions.length > 1 && (
                    <>
                        <hr />
                        {workout.avg_volume > 0 && (
                            <WorkoutVolumeChart sessions={workout.sessions} />
                        )}
                    </>
                )}
                {workout.session_count > 0 && (
                    <>
                        <hr />
                        <table>
                            <colgroup>
                                <col
                                    className={
                                        workout.avg_volume > 0
                                            ? "col-230"
                                            : "col-370"
                                    }
                                />
                                <col
                                    className={
                                        workout.avg_volume > 0
                                            ? "col-120"
                                            : "col-260"
                                    }
                                />
                                {workout.avg_volume > 0 && (
                                    <col className="col-280" />
                                )}
                            </colgroup>
                            <thead>
                                <tr>
                                    <th>{translate("date")}</th>
                                    <th>{translate("time")}</th>
                                    {workout.avg_volume > 0 && (
                                        <th>{translate("volume")}</th>
                                    )}
                                </tr>
                            </thead>
                            <tbody>
                                {workout.sessions.map((session, idx) => (
                                    <tr key={idx} className="divider-bottom">
                                        <td className="text-center">
                                            {formatTimeDate(session.date)}
                                        </td>
                                        <td className="text-center">
                                            {formatDuration(session.time)}
                                        </td>
                                        {workout.avg_volume > 0 && (
                                            <td className="text-center">
                                                {formatNumber(
                                                    fromKg(session.volume),
                                                    1,
                                                )}{" "}
                                                {getWeightUnit()}
                                                {idx <
                                                    workout.sessions.length -
                                                        1 &&
                                                    " (" +
                                                        formatPercent(
                                                            (workout.sessions[
                                                                idx
                                                            ].volume -
                                                                workout
                                                                    .sessions[
                                                                    idx + 1
                                                                ].volume) /
                                                                workout
                                                                    .sessions[
                                                                    idx + 1
                                                                ].volume,
                                                            2,
                                                            true,
                                                        ) +
                                                        ")"}
                                            </td>
                                        )}
                                    </tr>
                                ))}
                            </tbody>
                        </table>
                    </>
                )}
            </DialogContent>
        </Dialog>
    );
}
