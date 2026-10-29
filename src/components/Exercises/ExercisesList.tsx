import {
    ExerciseDetails,
    ExerciseListItem,
    ProgressionSuggestion,
} from "@/utils/backend/models";
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
import { Button } from "@mui/material";

export type ExerciseListEntry = ExerciseListItem & { used: boolean };

export function ExercisesList() {
    const loadingTask = useLoadingTask();
    const { sessionsVersion, exercisesCatalog } = useContext(AppContext);
    const { translate, formatDate } = useContext(I18nSettingsContext);
    const [exercises, setExercises] = useBackendList<ExerciseListEntry>(
        () =>
            BackendClient.getExercises().then((data) => {
                const exercises = data
                    .toSorted((a, b) => {
                        return translate(
                            "exercise_" + a.category + "_" + a.id,
                        ).localeCompare(
                            translate("exercise_" + b.category + "_" + b.id),
                        );
                    })
                    .map((e) => ({ ...e, used: true }));

                const used_keys = data.map((e) => e.category + "-" + e.id);
                const unused_exercises: ExerciseListEntry[] = [];
                exercisesCatalog.forEach((ec) => {
                    const key = ec.ex_cat + "-" + ec.ex_id;
                    if (!used_keys.includes(key)) {
                        unused_exercises.push({
                            category: ec.ex_cat,
                            id: ec.ex_id,
                            date: 0,
                            notes: null,
                            progression: false,
                            reps: 0,
                            unilateral: false,
                            weight: 0,
                            used: false,
                        });
                    }
                });

                return [
                    ...exercises,
                    ...unused_exercises.toSorted((a, b) => {
                        return translate(
                            "exercise_" + a.category + "_" + a.id,
                        ).localeCompare(
                            translate("exercise_" + b.category + "_" + b.id),
                        );
                    }),
                ];
            }),
        [sessionsVersion],
    );
    const [exerciseDetails, setExerciseDetails] = useState<
        ExerciseDetails | undefined
    >(undefined);
    const [suggestion, setSuggestion] = useState<ProgressionSuggestion | null>(
        null,
    );

    const pendingSuggestions =
        exercises.find((e) => e.progression) !== undefined;

    const getExerciseDetails = (category: number, id: number) => {
        loadingTask(
            Promise.all([
                BackendClient.getExerciseDetails(category, id),
                BackendClient.getProgressionSuggestion(category, id),
            ]),
        ).then(([details, suggestion]) => {
            setExerciseDetails(details);
            setSuggestion(suggestion);
        });
    };

    const setPendingProgression = (
        exercise: ExerciseDetails,
        pending: boolean,
    ) => {
        setExercises((prevs) =>
            prevs.map((cur) =>
                cur.category == exercise.category && cur.id == exercise.id
                    ? { ...cur, progression: pending }
                    : cur,
            ),
        );
    };

    const [filter, filteredExercises] = useFilterList(exercises, (exercise) => {
        return [
            translate("exercise_" + exercise.category + "_" + exercise.id),
            translate("category_" + exercise.category),
            formatDate(exercise.date),
        ];
    });

    const applyProgressions = async () => {
        const applied: ExerciseListItem[] = [];
        try {
            for (const exercise of exercises) {
                if (!exercise.progression) {
                    continue;
                }

                const progression = await loadingTask(
                    BackendClient.getProgressionSuggestion(
                        exercise.category,
                        exercise.id,
                    ),
                );

                if (progression) {
                    await loadingTask(
                        BackendClient.applyProgressionSuggestion(
                            exercise.category,
                            exercise.id,
                            progression,
                        ),
                    );
                    applied.push(exercise);
                }
            }
        } catch (e) {
            console.error("Error applying progression suggestions", e);
        } finally {
            // Reflect whatever was applied before a failure
            setExercises((prev) =>
                prev.map((e) =>
                    applied.some(
                        (a) => a.category == e.category && a.id == e.id,
                    )
                        ? { ...e, progression: false }
                        : e,
                ),
            );
        }
    };

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

            {pendingSuggestions && (
                <div className="list-action-bar list-action-bar-row">
                    <Button
                        id="add-measure-button"
                        variant="contained"
                        className="full-width-button"
                        onClick={applyProgressions}
                    >
                        {translate("apply_progression_suggestions")}
                    </Button>
                </div>
            )}

            <div>
                {exerciseDetails && (
                    <ExerciseModal
                        exercise={exerciseDetails}
                        suggestion={suggestion}
                        onProgressionApplied={() => {
                            setPendingProgression(exerciseDetails, false);
                        }}
                        onSuggestionChange={(sug) =>
                            setPendingProgression(exerciseDetails, sug != null)
                        }
                        onClose={() => setExerciseDetails(undefined)}
                    />
                )}
            </div>
        </>
    );
}
