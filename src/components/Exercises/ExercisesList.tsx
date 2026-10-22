import { ExerciseDetails, ExerciseListItem } from "@/utils/backend/models";
import { ExerciseModal } from "./ExerciseModal";
import { ExerciseRow } from "./helpers/ExerciseRow";
import { useContext, useState } from "react";
import { BackendClient } from "@/utils/backend/client";
import { AppContext } from "@/context/AppContext";
import { useBackendList } from "@/hooks/useBackendList";
import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { useFilterList } from "@/hooks/useFilterList";
import { useLoadingTask } from "@/hooks/useLoadingTask";
import "@/styles/Exercises/ExercisesList.css";

export function ExercisesList() {
    const loadingTask = useLoadingTask();
    const { sessionsVersion } = useContext(AppContext);
    const { translate, formatDate } = useContext(I18nSettingsContext);
    const [exercises, setExercises] = useBackendList<ExerciseListItem>(
        () =>
            BackendClient.getExercises().then((data) =>
                data.toSorted((a, b) => {
                    return translate(
                        "exercise_" + a.category + "_" + a.id,
                    ).localeCompare(
                        translate("exercise_" + b.category + "_" + b.id),
                    );
                }),
            ),
        [sessionsVersion],
    );
    const [exerciseDetails, setExerciseDetails] = useState<
        ExerciseDetails | undefined
    >(undefined);

    const getExerciseDetails = (category: number, id: number) => {
        loadingTask(BackendClient.getExerciseDetails(category, id)).then(
            (details) => {
                setExerciseDetails(details);
            },
        );
    };

    const appliedProgression = (exercise: ExerciseDetails) => {
        setExercises((prevs) =>
            prevs.map((cur) =>
                cur.category == exercise.category && cur.id == exercise.id
                    ? { ...cur, progression: false }
                    : cur,
            ),
        );
    };

    const [filter, filteredExercises] = useFilterList(exercises, (exercise) => {
        return [
            translate("exercise_" + exercise.category + "_" + exercise.id),
            translate("exercise_" + exercise.category),
            formatDate(exercise.date),
        ];
    });

    return (
        <>
            {filter}
            <table>
                <colgroup>
                    <col className="col-430" />
                    <col className="col-100" />
                    <col className="col-60" />
                    <col className="col-100" />
                </colgroup>
                <thead>
                    <tr>
                        <th className="text-center">{translate("exercise")}</th>
                        <th className="text-center">{translate("pr")}</th>
                        <th className="text-center">{translate("rm")}</th>
                        <th className="text-center">{translate("date")}</th>
                    </tr>
                </thead>
                <tbody>
                    {filteredExercises.map((exercise) => (
                        <ExerciseRow
                            key={`${exercise.category}_${exercise.id}`}
                            exercise={exercise}
                            onSelect={getExerciseDetails}
                        />
                    ))}
                </tbody>
            </table>

            <div>
                {exerciseDetails && (
                    <ExerciseModal
                        exercise={exerciseDetails}
                        onProgressionApplied={() => {
                            appliedProgression(exerciseDetails);
                        }}
                        onClose={() => setExerciseDetails(undefined)}
                    />
                )}
            </div>
        </>
    );
}
