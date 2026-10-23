import { AppContext } from "@/context/AppContext";
import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { BackendClient } from "@/utils/backend/client";
import { WorkoutDetails } from "@/utils/backend/models";
import { LeafStep, StepGroup, WorkoutUtils } from "@/utils/WorkoutUtils";
import { JSX, useContext, useEffect, useState } from "react";

import "@/styles/Workouts/WorkoutPrintView.css";
import NoPageBreak from "../Report/helpers/NoPageBreak";

type Props = {
    workout: string;
};

export default function WorkoutPrintView({
    workout: workout_name,
}: Props): JSX.Element {
    const { appReady } = useContext(AppContext);
    const { translate, fromKg, formatDuration, formatNumber, getWeightUnit } =
        useContext(I18nSettingsContext);
    const [workout, setWorkout] = useState<WorkoutDetails>();

    useEffect(() => {
        if (!appReady) return;
        BackendClient.getWorkoutDetails(workout_name).then((data) => {
            setWorkout(data);
        });
    }, [appReady, workout_name]);

    useEffect(() => {
        if (!workout) return;
        (async () => {
            await document.fonts.ready;
            await BackendClient.workoutPrintReady(workout.name);
        })();
    }, [workout]);

    if (!workout) {
        return <></>;
    }

    const steps = WorkoutUtils.parseWorkoutSteps(workout.steps, fromKg);

    return (
        <div className="workout-print">
            <h1>{workout.name}</h1>
            {workout.notes && <div id="workout-notes">{workout.notes}</div>}
            {steps.map((step, idx) => {
                if (WorkoutUtils.isStepGroup(step)) {
                    return groupToHtml(
                        step,
                        translate,
                        formatDuration,
                        formatNumber,
                        fromKg,
                        getWeightUnit,
                    );
                } else {
                    return leafToHtml(
                        idx,
                        step,
                        translate,
                        formatDuration,
                        formatNumber,
                        fromKg,
                        getWeightUnit,
                    );
                }
            })}
        </div>
    );
}

function groupToHtml(
    group: StepGroup,
    translate: (lit: string) => string,
    formatDuration: (time: number) => string,
    formatNumber: (time: number, dec: number) => string,
    fromKg: (weight: number) => number,
    getWeightUnit: () => string,
): JSX.Element {
    return (
        <NoPageBreak>
            <fieldset>
                <legend>
                    <b>{translate("repeat") + " x" + group.laps}</b>
                </legend>
                <table>
                    <colgroup>
                        <col className="col-name"></col>
                        <col className="col-reps"></col>
                        <col className="col-weight"></col>
                        <col className="col-time"></col>
                        <col className="col-notes"></col>
                    </colgroup>
                    <thead>
                        <tr>
                            <th></th>
                            <th className="centered">
                                {translate("repetitions")}
                            </th>
                            <th className="centered">{translate("weight")}</th>
                            <th className="centered">{translate("time")}</th>
                            <th className="centered">{translate("notes")}</th>
                        </tr>
                    </thead>
                    <tbody>
                        {group.wrapped.map((leaf, idx) =>
                            leafToHtml(
                                idx,
                                leaf,
                                translate,
                                formatDuration,
                                formatNumber,
                                fromKg,
                                getWeightUnit,
                            ),
                        )}
                    </tbody>
                </table>
            </fieldset>
        </NoPageBreak>
    );
}

function leafToHtml(
    idx: number,
    leaf: LeafStep,
    translate: (lit: string) => string,
    formatDuration: (time: number) => string,
    formatNumber: (time: number, dec: number) => string,
    fromKg: (weight: number) => number,
    getWeightUnit: () => string,
): JSX.Element {
    if (leaf.kind == "Rest") {
        return (
            <tr key={idx}>
                <td>
                    <b>{translate("step_type_rest")}</b>
                </td>
                <td></td>
                <td></td>
                <td className="centered">
                    {leaf.time === null && <>{translate("limit_type_lap")}</>}
                    {leaf.time !== null && <>{formatDuration(leaf.time)}</>}
                </td>
                <td>{leaf.notes !== null && <>{leaf.notes}</>}</td>
            </tr>
        );
    } else {
        return (
            <tr key={idx}>
                <td>
                    <b>
                        {translate(
                            "exercise_" + leaf.ex_cat + "_" + leaf.ex_id,
                        )}
                    </b>
                </td>
                <td className="centered">
                    {leaf.reps !== null && <>{leaf.reps}</>}
                </td>
                <td className="centered">
                    {leaf.weight !== null && (
                        <>
                            {formatNumber(fromKg(leaf.weight), 1) +
                                " " +
                                getWeightUnit()}
                        </>
                    )}
                </td>
                <td className="centered">
                    {leaf.time !== null && <>{formatDuration(leaf.time)}</>}
                </td>
                <td>{leaf.notes !== null && <>{leaf.notes}</>}</td>
            </tr>
        );
    }
}
