import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { ExerciseListItem, WorkoutDetails } from "@/utils/backend/models";
import { useContext, useState } from "react";
import {
    Accordion,
    AccordionDetails,
    AccordionSummary,
    Box,
    Dialog,
    DialogContent,
    DialogTitle,
    IconButton,
    Switch,
} from "@mui/material";
import CloseIcon from "@mui/icons-material/Close";
import ExpandMoreIcon from "@mui/icons-material/ExpandMore";
import { BackendClient } from "@/utils/backend/client";
import { useLoadingTask } from "@/hooks/useLoadingTask";
import { useBackendEvent } from "@/hooks/useBackendEvent";
import { WorkoutVolumeChart } from "../charts/WorkoutVolumeChart";
import { WorkoutActionsMenu } from "../menus/WorkoutActionsMenu";
import { BackendListener } from "@/utils/backend/listener";
import { MuscleModel } from "../../Exercises/charts/MuscleModel";
import { WorkoutStructure } from "../views/WorkoutStructure";

type Props = {
    workout: WorkoutDetails;
    showEnable: boolean;
    onClose: () => void;
    onEnabledChange?: (suggest: boolean) => void;
    onUpdate?: (id: number, status: boolean) => void;
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
    onEnabledChange: onSuggestChange = undefined,
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
    const [enabled, setEnabled] = useState(initialWorkout.enabled);
    // Needed by the structure for unilateral reps and inherited notes, loaded on first expand
    const [exercises, setExercises] = useState<ExerciseListItem[]>();

    // Reset to the new prop during render instead of in an effect
    const [prevInitialWorkout, setPrevInitialWorkout] =
        useState(initialWorkout);
    if (prevInitialWorkout !== initialWorkout) {
        setPrevInitialWorkout(initialWorkout);
        setWorkout(initialWorkout);
    }

    useBackendEvent(BackendListener.onAddedWorkoutSteps, (ids) => {
        if (ids.includes(initialWorkout.id)) {
            withLoading(
                BackendClient.getWorkoutDetails(initialWorkout.id),
            ).then(setWorkout);
        }
    });

    const toggleEnabled = () => {
        withLoading(BackendClient.setWorkoutStatus(workout.id, !enabled)).then(
            () => {
                if (onUpdate) {
                    onUpdate(workout.id, !enabled);
                }
                onSuggestChange!(!enabled);
                setEnabled((prev) => !prev);
            },
        );
    };

    const onStructureToggle = (_: React.SyntheticEvent, expanded: boolean) => {
        if (expanded && !exercises) {
            withLoading(BackendClient.getExercises()).then(setExercises);
        }
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
                                workoutId={workout.id}
                                onOpenEdit={() => onOpenEdit(workout)}
                                onOpenDuplicate={() => onOpenDuplicate(workout)}
                                send={workout.enabled}
                            />
                        )}
                        {!onOpenDuplicate && (
                            <WorkoutActionsMenu
                                workoutId={workout.id}
                                onOpenEdit={() => onOpenEdit(workout)}
                                send={workout.enabled}
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
                        <div className="session-details-row">
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
                                                <td>
                                                    {translate("sessions")}:
                                                </td>
                                                <td>
                                                    {formatNumber(
                                                        workout.session_count,
                                                        0,
                                                    )}
                                                </td>
                                            </tr>
                                            <tr>
                                                <td>
                                                    {translate(
                                                        "latest_session",
                                                    )}
                                                </td>
                                                <td>
                                                    {workout.latest_session &&
                                                        formatTimeDate(
                                                            workout.latest_session,
                                                        )}
                                                </td>
                                            </tr>
                                            <tr>
                                                <td>
                                                    {translate("average_time")}
                                                </td>
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
                                            <td>{translate("enabled")}:</td>
                                            <td>
                                                <Switch
                                                    checked={enabled}
                                                    onChange={toggleEnabled}
                                                    slotProps={{
                                                        input: {
                                                            "aria-label":
                                                                translate(
                                                                    "enabled",
                                                                ),
                                                        },
                                                    }}
                                                />
                                            </td>
                                        </tr>
                                    )}
                                </tbody>
                            </table>
                            {(workout.primary_muscles.length > 0 ||
                                workout.secondary_muscles.length > 0) && (
                                <div className="session-muscle-model">
                                    <MuscleModel
                                        primary={workout.primary_muscles}
                                        secondary={workout.secondary_muscles}
                                    />
                                </div>
                            )}
                        </div>
                    </div>
                </div>
                {workout.steps.length > 0 && (
                    <Accordion
                        disableGutters
                        onChange={onStructureToggle}
                        slotProps={{ transition: { unmountOnExit: true } }}
                        sx={{
                            backgroundColor: "transparent",
                            backgroundImage: "none",
                            boxShadow: "none",
                            border: "1px solid #e4e4e450",
                            borderRadius: "4px",
                            overflow: "hidden",
                            "&::before": { display: "none" },
                        }}
                    >
                        <AccordionSummary
                            expandIcon={<ExpandMoreIcon />}
                            sx={{
                                backgroundColor: "#ffffff0d",
                                "&:hover": { backgroundColor: "#ffffff1a" },
                                "&.Mui-expanded": {
                                    borderBottom: "1px solid #e4e4e450",
                                },
                            }}
                        >
                            <b>{translate("workout_structure")}</b>
                        </AccordionSummary>
                        <AccordionDetails>
                            {exercises && (
                                <WorkoutStructure
                                    workout={workout}
                                    exercises={exercises}
                                    notesAsTooltip
                                />
                            )}
                        </AccordionDetails>
                    </Accordion>
                )}
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
