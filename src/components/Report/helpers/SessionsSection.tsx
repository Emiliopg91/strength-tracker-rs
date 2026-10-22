import { JSX, useContext } from "react";
import PageContent from "./PageContent";
import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { ExerciseDetails, SessionDetails } from "@/utils/backend/models";
import NoPageBreak from "./NoPageBreak";
import { SessionUtils } from "@/utils/SessionUtils";

type Props = {
    sessions: SessionDetails[];
    exercises: ExerciseDetails[];
};

export default function SessionsSection({
    sessions,
    exercises: exercises_details,
}: Props): JSX.Element {
    const { translate, formatTimeDate, formatDuration, fromKg, getWeightUnit } =
        useContext(I18nSettingsContext);

    return (
        <PageContent>
            <h2>{translate("sessions")}</h2>
            <div className="section-container">
                {sessions.map((session) => {
                    const { exercises, groupedSeries } =
                        SessionUtils.groupByExercises(session.sets);
                    return (
                        <NoPageBreak key={session.timestamp}>
                            <h4>
                                {session.name +
                                    " @ " +
                                    formatTimeDate(session.timestamp)}
                            </h4>
                            <div className="section-container">
                                <div className="report-table">
                                    <b>{translate("sport") + ": "}</b>
                                    {translate("sport_" + session.sport) +
                                        " - " +
                                        translate(
                                            "sport_" +
                                                session.sport +
                                                "_" +
                                                session.sub_sport,
                                        )}
                                    <br />
                                    <b>{translate("time") + ": "}</b>
                                    {formatDuration(session.active_time) +
                                        " / " +
                                        formatDuration(
                                            session.total_elapsed_time,
                                        )}
                                    <br />
                                    <b>{translate("active_calories") + ": "}</b>
                                    {session.active_calories}
                                    <br />
                                    <b>{translate("workout_load") + ": "}</b>
                                    {session.training_load}
                                </div>
                                <br />
                                <div className="report-table">
                                    <table>
                                        <thead>
                                            <tr>
                                                <th>{translate("exercise")}</th>
                                                <th>
                                                    {translate("repetitions")}
                                                </th>
                                                <th>{translate("weight")}</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {exercises.map((exercise) => {
                                                let name = translate(
                                                    "exercise_" +
                                                        exercise.replace(
                                                            "-",
                                                            "_",
                                                        ),
                                                );
                                                if (
                                                    exercises_details.find(
                                                        (e) =>
                                                            e.category +
                                                                "-" +
                                                                e.id ==
                                                            exercise,
                                                    )?.unilateral
                                                ) {
                                                    name =
                                                        name +
                                                        " (" +
                                                        translate(
                                                            "unilateral",
                                                        ) +
                                                        ")";
                                                }

                                                return (
                                                    <>
                                                        {groupedSeries[
                                                            exercise
                                                        ].map(
                                                            ([, set], idx) => (
                                                                <tr
                                                                    key={
                                                                        exercise
                                                                    }
                                                                >
                                                                    {idx ===
                                                                        0 && (
                                                                        <td
                                                                            rowSpan={
                                                                                groupedSeries[
                                                                                    exercise
                                                                                ]
                                                                                    .length
                                                                            }
                                                                        >
                                                                            {
                                                                                name
                                                                            }
                                                                        </td>
                                                                    )}
                                                                    <td>
                                                                        {
                                                                            set.reps
                                                                        }
                                                                    </td>
                                                                    <td>
                                                                        {fromKg(
                                                                            set.weight,
                                                                        ) +
                                                                            " " +
                                                                            getWeightUnit()}
                                                                    </td>
                                                                </tr>
                                                            ),
                                                        )}
                                                    </>
                                                );
                                            })}
                                        </tbody>
                                    </table>
                                </div>
                            </div>
                        </NoPageBreak>
                    );
                })}
            </div>
        </PageContent>
    );
}
