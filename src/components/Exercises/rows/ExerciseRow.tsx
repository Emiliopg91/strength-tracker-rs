import { AppContext } from "@/context/AppContext";
import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { SessionUtils } from "@/utils/SessionUtils";
import { useContext } from "react";
import PriorityHighIcon from "@mui/icons-material/PriorityHigh";
import { Tooltip } from "@mui/material";
import { ExerciseListEntry } from "../ExercisesList";

type Props = {
    exercise: ExerciseListEntry;
    onSelect: (category: number, id: number) => void;
};

export function ExerciseRow({ exercise, onSelect }: Props) {
    const { translate, fromKg, getWeightUnit, formatDate, formatNumber } =
        useContext(I18nSettingsContext);
    const { exerciseName } = useContext(AppContext);

    return (
        <tr
            className={`clickable-row ${exercise.relevant ? "" : "unused-exercise"}`}
            onClick={() => onSelect(exercise.category, exercise.id)}
        >
            <td className="text-left" style={{ paddingLeft: "5px" }}>
                {exercise.progression && (
                    <Tooltip title={translate("progression_suggested")}>
                        <PriorityHighIcon className="warn-icon" />
                    </Tooltip>
                )}
                {exerciseName(exercise.category, exercise.id)}
            </td>
            <td>
                {exercise.used &&
                    exercise.reps +
                        "x" +
                        formatNumber(fromKg(exercise.weight), 1) +
                        " " +
                        getWeightUnit()}
            </td>
            <td>
                {exercise.used &&
                    formatNumber(
                        fromKg(
                            SessionUtils.estimate1rm(
                                exercise.weight,
                                exercise.reps,
                                exercise.unilateral,
                            ),
                        ),
                        0,
                    ) +
                        " " +
                        getWeightUnit()}
            </td>
            <td>{exercise.used && formatDate(exercise.date)}</td>
        </tr>
    );
}
