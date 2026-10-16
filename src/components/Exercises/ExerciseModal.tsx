import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { ExerciseDetails } from "@/utils/backend/models";
import { useContext } from "react";
import { Dialog, DialogContent, DialogTitle, IconButton } from "@mui/material";
import CloseIcon from "@mui/icons-material/Close";
import { ExerciseVolumeChart } from "./helpers/ExerciseVolumeChart";
import { SessionUtils } from "@/utils/SessionUtils";
import EmojiEventsIcon from "@mui/icons-material/EmojiEvents";

type Props = {
    exercise: ExerciseDetails;
    onClose: () => void;
};

export function ExerciseModal({ exercise, onClose }: Props) {
    const { translate, fromKg, getWeightUnit, formatTimeDate, formatNumber } =
        useContext(I18nSettingsContext);

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
                    </tbody>
                </table>
                {Object.keys(exercise.workouts).length > 1 && (
                    <>
                        <hr />
                        <ExerciseVolumeChart series={exercise.series} />
                        <br />
                    </>
                )}
                <hr />
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
