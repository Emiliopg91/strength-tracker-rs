import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { ExerciseDetails, ProgressionSuggestion } from "@/utils/backend/models";
import { useContext, useEffect, useEffectEvent, useRef, useState } from "react";
import {
    Button,
    Dialog,
    DialogContent,
    DialogTitle,
    IconButton,
    Switch,
    TextField,
} from "@mui/material";
import CloseIcon from "@mui/icons-material/Close";
import { ExerciseVolumeChart } from "./helpers/ExerciseVolumeChart";
import { SessionUtils } from "@/utils/SessionUtils";
import EmojiEventsIcon from "@mui/icons-material/EmojiEvents";
import { useLoadingTask } from "@/hooks/useLoadingTask";
import { BackendClient } from "@/utils/backend/client";

type Props = {
    exercise: ExerciseDetails;
    suggestion: ProgressionSuggestion | null;
    onProgressionApplied: () => void;
    onClose: () => void;
};

export function ExerciseModal({
    exercise,
    suggestion: suggestionInput,
    onClose,
    onProgressionApplied,
}: Props) {
    const { translate, fromKg, getWeightUnit, formatTimeDate, formatNumber } =
        useContext(I18nSettingsContext);

    const [unilateral, setUnilateral] = useState(exercise.unilateral);
    const [progression, setProgression] = useState(exercise.progression);
    const [suggestion, setSuggestion] = useState<ProgressionSuggestion | null>(
        suggestionInput,
    );
    const [range, setRange] = useState(exercise.reps_range);
    // Value being edited as "N-N-...-N"; the range above only changes once saved
    const [rangeInput, setRangeInput] = useState(exercise.reps_range);
    const parsedRange = (() => {
        if (!/^\d+(-\d+)+$/.test(rangeInput)) {
            return null;
        }
        const steps = rangeInput.split("-").map((s) => parseInt(s));
        const valid =
            steps[0] > 0 && steps.every((s, i) => i == 0 || s > steps[i - 1]);
        return valid ? steps : null;
    })();
    const loadingTask = useLoadingTask();

    const fetchSuggestion = useEffectEvent(() => {
        loadingTask(
            BackendClient.getProgressionSuggestion(
                exercise.category,
                exercise.id,
            ),
        ).then(setSuggestion);
    });

    const settingsKey = `${exercise.category}|${exercise.id}|${progression}|${unilateral}|${range}`;
    // Settings the current suggestion was fetched for; the initial one comes from the parent
    const fetchedFor = useRef<string | null>(settingsKey);

    // Debounced so a burst of setting changes only fetches once
    useEffect(() => {
        if (!progression) {
            fetchedFor.current = null;
            return;
        }
        if (fetchedFor.current === settingsKey) {
            return;
        }

        const timer = setTimeout(() => {
            fetchedFor.current = settingsKey;
            fetchSuggestion();
        }, 200);
        return () => clearTimeout(timer);
    }, [settingsKey, progression]);

    const toggleUnilateral = () => {
        loadingTask(
            BackendClient.setExerciseUnilateral(
                exercise.category,
                exercise.id,
                !unilateral,
            ),
        ).then(() => {
            setUnilateral((prev) => !prev);
        });
    };

    const applySuggestion = () => {
        loadingTask(
            BackendClient.applyProgressionSuggestion(
                exercise.category,
                exercise.id,
                suggestion!,
            ),
        ).then(() => {
            setSuggestion(null);
            onProgressionApplied();
        });
    };

    const toggleProgression = () => {
        loadingTask(
            BackendClient.setExerciseProgression(
                exercise.category,
                exercise.id,
                !progression,
            ),
        ).then(() => {
            setProgression((prev) => !prev);
        });
    };

    // Keystrokes that break the "N-N-...-N" pattern are dropped
    const onRangeChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        const value = e.target.value;
        if (/^\d{0,3}(-\d{0,3})*$/.test(value)) {
            setRangeInput(value);
        }
    };

    const updateRepsRange = () => {
        if (!parsedRange) {
            return;
        }
        const normalized = parsedRange.join("-");

        setRangeInput(normalized);

        loadingTask(
            BackendClient.setExerciseRepsRange(
                exercise.category,
                exercise.id,
                normalized,
            ),
        ).then(() => {
            setRange(normalized);
        });
    };

    return (
        <Dialog open={true} onClose={onClose} fullWidth maxWidth="sm">
            <DialogTitle>
                {translate("exercise_" + exercise.category + "_" + exercise.id)}
                <IconButton onClick={onClose} className="modal-close-button">
                    <CloseIcon />
                </IconButton>
            </DialogTitle>

            <DialogContent dividers>
                <table id="workout-details-table">
                    <colgroup>
                        <col className="col-200" />
                        <col />
                        <col />
                    </colgroup>
                    <tbody>
                        <tr>
                            <td>{translate("category")}:</td>
                            <td>
                                {translate("exercise_" + exercise.category)}
                            </td>
                        </tr>
                        <tr>
                            <td>{translate("rm")}:</td>
                            <td>
                                {formatNumber(
                                    fromKg(
                                        SessionUtils.estimate1rm(
                                            exercise.weight,
                                            exercise.reps,
                                            unilateral,
                                        ),
                                    ),
                                    0,
                                ) +
                                    " " +
                                    getWeightUnit()}
                            </td>
                        </tr>
                        <tr>
                            <td>{translate("personal_record")}:</td>
                            <td>
                                {exercise.reps +
                                    "x" +
                                    formatNumber(fromKg(exercise.weight), 1) +
                                    " " +
                                    getWeightUnit()}
                            </td>
                        </tr>
                        <tr>
                            <td>{translate("record_date")}:</td>
                            <td>{formatTimeDate(exercise.pr_date)}</td>
                        </tr>
                        <tr>
                            <td>{translate("unilateral")}:</td>
                            <td>
                                <Switch
                                    checked={unilateral}
                                    onChange={toggleUnilateral}
                                    slotProps={{
                                        input: {
                                            "aria-label":
                                                translate("unilateral"),
                                        },
                                    }}
                                />
                            </td>
                        </tr>
                        <tr>
                            <td>{translate("progression_suggestion")}</td>
                            <td>
                                <Switch
                                    checked={progression}
                                    onChange={toggleProgression}
                                    slotProps={{
                                        input: {
                                            "aria-label": translate(
                                                "progression_suggestion",
                                            ),
                                        },
                                    }}
                                />
                            </td>
                        </tr>
                        {progression && (
                            <tr>
                                <td>{translate("reps_range")}</td>
                                <td>
                                    <TextField
                                        size="small"
                                        error={parsedRange == null}
                                        value={rangeInput}
                                        placeholder="8-10-12"
                                        sx={{ width: 140 }}
                                        slotProps={{
                                            htmlInput: {
                                                inputMode: "numeric",
                                            },
                                        }}
                                        onChange={onRangeChange}
                                    />
                                    <Button
                                        onClick={updateRepsRange}
                                        disabled={parsedRange == null}
                                        variant="contained"
                                        sx={{ ml: 1, mt: 0.25 }}
                                    >
                                        {translate("apply")}
                                    </Button>
                                </td>
                            </tr>
                        )}
                    </tbody>
                </table>
                {progression && suggestion && (
                    <>
                        <hr />
                        <table className="full-width">
                            <tbody>
                                <tr>
                                    <td>
                                        <>
                                            {translate(
                                                "exercises_requires_progression",
                                                [
                                                    suggestion.reps.toString(),
                                                    formatNumber(
                                                        fromKg(
                                                            suggestion.weight,
                                                        ),
                                                        1,
                                                    ),
                                                    getWeightUnit(),
                                                ],
                                            )}
                                        </>
                                    </td>
                                    <td>
                                        <Button
                                            variant="contained"
                                            onClick={applySuggestion}
                                        >
                                            {translate("apply_suggestion")}
                                        </Button>
                                    </td>
                                </tr>
                            </tbody>
                        </table>
                    </>
                )}
                {Object.keys(exercise.workouts).length > 1 && (
                    <>
                        <hr />
                        <ExerciseVolumeChart
                            series={exercise.series}
                            exercise={exercise}
                        />
                    </>
                )}
                <hr />
                <table className="full-width">
                    <colgroup>
                        <col className="col-350" />
                        <col />
                        <col />
                    </colgroup>
                    <thead>
                        <tr>
                            <th>{translate("workout")}</th>
                            <th></th>
                            <th>{translate("series")}</th>
                        </tr>
                    </thead>
                    <tbody>
                        {exercise.workouts.map((workout) => {
                            return exercise.series[workout].map(
                                (serie, idx) => (
                                    <tr key={`${workout}-${idx}`}>
                                        {idx === 0 && (
                                            <td
                                                className="divider-bottom"
                                                rowSpan={
                                                    exercise.series[workout]
                                                        .length
                                                }
                                            >
                                                {workout.split("\n")[0] +
                                                    " @ " +
                                                    formatTimeDate(
                                                        parseInt(
                                                            workout.split(
                                                                "\n",
                                                            )[1],
                                                        ),
                                                    )}
                                            </td>
                                        )}
                                        <td
                                            className={
                                                idx ===
                                                exercise.series[workout]
                                                    .length -
                                                    1
                                                    ? "divider-bottom"
                                                    : ""
                                            }
                                            style={{ textAlign: "right" }}
                                        >
                                            {serie.pr && (
                                                <EmojiEventsIcon
                                                    fontSize="small"
                                                    className="trophy-icon"
                                                />
                                            )}
                                        </td>
                                        <td
                                            className={`text-center${
                                                idx ===
                                                exercise.series[workout]
                                                    .length -
                                                    1
                                                    ? " divider-bottom"
                                                    : ""
                                            }`}
                                        >
                                            {serie.reps}x
                                            {formatNumber(
                                                fromKg(serie.weight),
                                                1,
                                            ) +
                                                " " +
                                                getWeightUnit()}
                                        </td>
                                    </tr>
                                ),
                            );
                        })}
                    </tbody>
                </table>
            </DialogContent>
        </Dialog>
    );
}
