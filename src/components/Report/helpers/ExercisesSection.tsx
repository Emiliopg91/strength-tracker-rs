import { AppContext } from "@/context/AppContext";
import { JSX, useContext } from "react";
import PageContent from "./PageContent";
import NoPageBreak from "./NoPageBreak";
import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { ExerciseDetails } from "@/utils/backend/models";

type Props = {
    exercises: ExerciseDetails[];
};

export default function ExercisesSection({ exercises }: Props): JSX.Element {
    const { translate, formatTimeDate, fromKg, getWeightUnit, formatNumber } =
        useContext(I18nSettingsContext);
    const { exerciseName } = useContext(AppContext);

    return (
        <>
            {exercises.length > 0 && (
                <PageContent>
                    <h2>{translate("exercises")}</h2>

                    <div className="section-container">
                        {exercises.map((exercise) => (
                            <NoPageBreak
                                key={exercise.category + "-" + exercise.id}
                            >
                                <h4>
                                    {exerciseName(
                                        exercise.category,
                                        exercise.id,
                                    )}{" "}
                                    {exercise.unilateral && (
                                        <>
                                            {" (" +
                                                translate("unilateral") +
                                                ")"}
                                        </>
                                    )}
                                </h4>
                                <div className="section-container">
                                    <div className="report-table">
                                        <table>
                                            <thead>
                                                <tr>
                                                    <th>
                                                        {translate("workout")}
                                                    </th>
                                                    <th>
                                                        {translate(
                                                            "repetitions",
                                                        )}
                                                    </th>
                                                    <th>
                                                        {translate("weight")}
                                                    </th>
                                                </tr>
                                            </thead>
                                            <tbody>
                                                {exercise.workouts.map(
                                                    (workout) => {
                                                        return exercise.sessions[
                                                            workout
                                                        ].series.map(
                                                            (serie, idx) => (
                                                                <tr
                                                                    key={`${exercise.category}-${exercise.id}-${serie}-${idx}`}
                                                                >
                                                                    {idx ===
                                                                        0 && (
                                                                        <td
                                                                            rowSpan={
                                                                                exercise
                                                                                    .sessions[
                                                                                    workout
                                                                                ]
                                                                                    .series
                                                                                    .length
                                                                            }
                                                                        >
                                                                            {workout.split(
                                                                                "\n",
                                                                            )[0] +
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
                                                                    <td>
                                                                        {
                                                                            serie.reps
                                                                        }
                                                                    </td>
                                                                    <td>
                                                                        {formatNumber(
                                                                            fromKg(
                                                                                serie.weight ??
                                                                                    0,
                                                                            ),
                                                                            1,
                                                                        ) +
                                                                            " " +
                                                                            getWeightUnit()}
                                                                    </td>
                                                                </tr>
                                                            ),
                                                        );
                                                    },
                                                )}
                                            </tbody>
                                        </table>
                                    </div>
                                </div>
                            </NoPageBreak>
                        ))}
                    </div>
                </PageContent>
            )}
        </>
    );
}
