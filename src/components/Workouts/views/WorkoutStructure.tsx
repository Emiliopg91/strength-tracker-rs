import { AppContext } from "@/context/AppContext";
import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import {
    ExerciseListItem,
    StepType,
    WorkoutDetails,
} from "@/utils/backend/models";
import { LeafStep, StepGroup, WorkoutUtils } from "@/utils/WorkoutUtils";
import { JSX, ReactNode, useContext } from "react";
import { Tooltip } from "@mui/material";
import NoPageBreak from "@/components/Report/layout/NoPageBreak";

import "@/styles/Workouts/WorkoutStructure.css";

type Props = {
    workout: WorkoutDetails;
    exercises: ExerciseListItem[];
    /** Shows step notes as a tooltip over their row instead of in a column. */
    notesAsTooltip?: boolean;
};

type Formatters = {
    exercises: ExerciseListItem[];
    unilaterals: ExerciseListItem[];
    translate: (lit: string) => string;
    exerciseName: (ex_cat: number | null, ex_id: number | null) => string;
    formatDuration: (time: number) => string;
    formatNumber: (time: number, dec: number) => string;
    fromKg: (weight: number) => number;
    getWeightUnit: () => string;
    notesAsTooltip: boolean;
    /** Exercise notes only reach the steps of workouts with progression. */
    inheritNotes: boolean;
};

/** Notes and steps of a workout, as repeat blocks and step tables. */
export function WorkoutStructure({
    workout,
    exercises,
    notesAsTooltip = false,
}: Props): JSX.Element {
    const { exerciseName } = useContext(AppContext);
    const { translate, fromKg, formatDuration, formatNumber, getWeightUnit } =
        useContext(I18nSettingsContext);

    const steps = WorkoutUtils.parseWorkoutSteps(workout.steps, fromKg);
    const fmt: Formatters = {
        exercises,
        unilaterals: exercises.filter((e) => e.unilateral),
        translate,
        exerciseName,
        formatDuration,
        formatNumber,
        fromKg,
        getWeightUnit,
        notesAsTooltip,
        inheritNotes: workout.progression,
    };

    // Consecutive steps outside any repeat share a table, so their rows never end up loose
    const blocks: JSX.Element[] = [];
    let loose: LeafStep[] = [];
    const flushLoose = () => {
        if (loose.length > 0) {
            blocks.push(
                <NoPageBreak key={blocks.length}>
                    {stepsTable(
                        loose.map((leaf, idx) => leafToHtml(idx, leaf, fmt)),
                        fmt,
                    )}
                </NoPageBreak>,
            );
            loose = [];
        }
    };
    for (const step of steps) {
        if (WorkoutUtils.isStepGroup(step)) {
            flushLoose();
            blocks.push(groupToHtml(blocks.length, step, fmt));
        } else {
            loose.push(step);
        }
    }
    flushLoose();

    return (
        <div
            className={
                "workout-structure" +
                (notesAsTooltip ? " notes-as-tooltip" : "")
            }
        >
            {workout.notes && <div id="workout-notes">{workout.notes}</div>}
            {blocks}
        </div>
    );
}

function stepsTable(rows: JSX.Element[], fmt: Formatters): JSX.Element {
    const { translate } = fmt;
    return (
        <table>
            <colgroup>
                <col className="col-name"></col>
                <col className="col-reps"></col>
                <col className="col-weight"></col>
                <col className="col-time"></col>
                {!fmt.notesAsTooltip && <col className="col-notes"></col>}
            </colgroup>
            <thead>
                <tr>
                    <th></th>
                    <th className="centered">{translate("repetitions")}</th>
                    <th className="centered">{translate("weight")}</th>
                    <th className="centered">{translate("time")}</th>
                    {!fmt.notesAsTooltip && <th>{translate("notes")}</th>}
                </tr>
            </thead>
            <tbody>{rows}</tbody>
        </table>
    );
}

function groupToHtml(
    idx: number,
    group: StepGroup,
    fmt: Formatters,
): JSX.Element {
    return (
        <NoPageBreak key={idx}>
            <fieldset>
                <legend>
                    <b>{fmt.translate("repeat") + " x" + group.laps}</b>
                </legend>
                {stepsTable(
                    group.wrapped.map((child, childIdx) =>
                        WorkoutUtils.isStepGroup(child) ? (
                            <tr key={childIdx}>
                                <td colSpan={fmt.notesAsTooltip ? 4 : 5}>
                                    {groupToHtml(childIdx, child, fmt)}
                                </td>
                            </tr>
                        ) : (
                            leafToHtml(childIdx, child, fmt)
                        ),
                    ),
                    fmt,
                )}
            </fieldset>
        </NoPageBreak>
    );
}

function leafToHtml(idx: number, leaf: LeafStep, fmt: Formatters): JSX.Element {
    const {
        exercises,
        unilaterals,
        translate,
        formatDuration,
        formatNumber,
        fromKg,
        getWeightUnit,
    } = fmt;

    if (leaf.kind == "Rest") {
        return stepRow(
            idx,
            <>
                <td>
                    <b>{translate("step_type_rest")}</b>
                </td>
                <td></td>
                <td></td>
                <td className="centered">
                    {leaf.time === null && <>{translate("limit_type_lap")}</>}
                    {leaf.time !== null && <>{formatDuration(leaf.time)}</>}
                </td>
            </>,
            leaf.notes,
            fmt,
        );
    }

    const unilateral =
        unilaterals.find(
            (e) => e.category == leaf.ex_cat && e.id == leaf.ex_id,
        ) !== undefined;
    const notes =
        leaf.notes ||
        (fmt.inheritNotes
            ? exercises.find(
                  (e) => e.category == leaf.ex_cat && e.id == leaf.ex_id,
              )?.notes
            : null);

    return stepRow(
        idx,
        <>
            <td>
                <b>{stepName(leaf, fmt)}</b>
            </td>
            <td className="centered">
                {leaf.reps !== null && (
                    <>
                        {unilateral && "2x" + leaf.reps / 2}
                        {!unilateral && leaf.reps}
                    </>
                )}
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
        </>,
        notes,
        fmt,
    );
}

/** Exercise name, prefixed with the step type for warm-up, cool-down and other non-exercise steps. */
function stepName(leaf: LeafStep, fmt: Formatters): string {
    const type = fmt.translate("step_type_" + leaf.kind.toLowerCase());
    if (leaf.ex_cat == null) {
        return type;
    }
    const name = fmt.exerciseName(leaf.ex_cat, leaf.ex_id);
    return leaf.kind == StepType.Exercise ? name : type + " - " + name;
}

function stepRow(
    idx: number,
    cells: ReactNode,
    notes: string | null | undefined,
    fmt: Formatters,
): JSX.Element {
    if (!fmt.notesAsTooltip) {
        return (
            <tr key={idx}>
                {cells}
                <td>{notes}</td>
            </tr>
        );
    }
    if (!notes) {
        return <tr key={idx}>{cells}</tr>;
    }
    return (
        <Tooltip
            key={idx}
            title={<span className="workout-step-notes">{notes}</span>}
            placement="top"
            followCursor
        >
            <tr className="has-notes">{cells}</tr>
        </Tooltip>
    );
}
