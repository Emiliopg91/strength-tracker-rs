import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { ExerciseDetails, ProgressionSuggestion } from "@/utils/backend/models";
import { useContext, useEffect, useEffectEvent, useState } from "react";
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
    onProgressionApplied: () => void;
    onClose: () => void;
};

export function ExerciseModal({
    exercise,
    onClose,
    onProgressionApplied,
}: Props) {
    const { translate, fromKg, getWeightUnit, formatTimeDate, formatNumber } =
        useContext(I18nSettingsContext);

    const [unilateral, setUnilateral] = useState(exercise.unilateral);
    const [progression, setProgression] = useState(exercise.progression);
    const [suggestion, setSuggestion] = useState<ProgressionSuggestion | null>(
        null,
    );
    const [minRange, setMinRange] = useState(exercise.min_range);
    const [maxRange, setMaxRange] = useState(exercise.max_range);
    // Value being edited as "MIN-MAX"; the range above only changes once saved
    const [rangeInput, setRangeInput] = useState(
        `${exercise.min_range}-${exercise.max_range}`,
    );
    const parsedRange = (() => {
        const match = /^(\d+)-(\d+)$/.exec(rangeInput);
        if (!match) {
            return null;
        }
        const [min, max] = [parseInt(match[1]), parseInt(match[2])];
        return min > 0 && min < max ? { min, max } : null;
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

    // Debounced so a burst of setting changes only fetches once
    useEffect(() => {
        if (!progression) {
            return;
        }

        const timer = setTimeout(() => fetchSuggestion(), 200);
        return () => clearTimeout(timer);
    }, [
        exercise.category,
        exercise.id,
        progression,
        unilateral,
        minRange,
        maxRange,
    ]);

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

    // Keystrokes that break the "MIN-MAX" pattern are dropped
    const onRangeChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        const value = e.target.value;
        if (/^\d{0,3}(-\d{0,3})?$/.test(value)) {
            setRangeInput(value);
        }
    };

    const updateRepsRange = () => {
        if (!parsedRange) {
            return;
        }
        const { min, max } = parsedRange;

        setRangeInput(`${min}-${max}`);

        loadingTask(
            BackendClient.setExerciseRepsRange(
                exercise.category,
                exercise.id,
                min,
                max,
            ),
        ).then(() => {
            setMinRange(min);
            setMaxRange(max);
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
                                        placeholder="8-12"
                                        sx={{ width: 100 }}
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
                {Object.keys(exercise.workouts).length > 1 && (
                    <>
                        <hr />
                        <ExerciseVolumeChart
                            series={exercise.series}
                            exercise={exercise}
                        />
                        <br />
                    </>
                )}
                <hr />
                {progression && suggestion && (
                    <>
                        <table className="full-width">
                            <tbody>
                                <tr>
                                    <td style={{ paddingLeft: "15px" }}>
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
                                            style={{ marginLeft: "20px" }}
                                            onClick={applySuggestion}
                                        >
                                            {translate("apply_suggestion")}
                                        </Button>
                                    </td>
                                </tr>
                            </tbody>
                        </table>
                        <hr />
                    </>
                )}
                <table className="full-width">
                    <colgroup>
                        <col className="col-350" />
                        <col />
                        <col />
                        <col />
                    </colgroup>
                    <thead>
                        <tr>
                            <th>{translate("workout")}</th>
                            <th></th>
                            <th>{translate("series")}</th>
                            <th>{translate("rm")}</th>
                        </tr>
                    </thead>
                    <tbody>
                        {exercise.workouts.map((workout) => {
                            const e1rm = SessionUtils.maxE1rm(
                                exercise.series[workout],
                                unilateral,
                            );

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

                                        {idx === 0 && (
                                            <td
                                                className="divider-bottom"
                                                rowSpan={
                                                    exercise.series[workout]
                                                        .length
                                                }
                                                style={{ textAlign: "center" }}
                                            >
                                                {formatNumber(fromKg(e1rm), 0) +
                                                    " " +
                                                    getWeightUnit()}
                                            </td>
                                        )}
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
