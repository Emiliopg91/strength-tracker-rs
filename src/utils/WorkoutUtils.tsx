import { StepType, WorkoutStep } from "./backend/models";

export type LeafStep = Omit<WorkoutStep, "begin_idx" | "idx" | "workout">;

export interface StepGroup {
    laps: number;
    wrapped: LeafStep[];
}

export type WorkoutStepData = LeafStep | StepGroup;

// Per-field flags for a leaf, true when that field would block saving
export interface LeafErrors {
    time: boolean;
    exercise: boolean;
    weight: boolean;
    reps: boolean;
    notes: string;
}

export class WorkoutUtils {
    public static isStepGroup(step: WorkoutStepData): step is StepGroup {
        return "wrapped" in step;
    }

    public static parseWorkoutSteps(
        steps: WorkoutStep[],
        fromKg: (kg: number) => number,
    ): WorkoutStepData[] {
        const elements: WorkoutStepData[] = [];

        let localSteps = steps.toSorted((a, b) => a.idx - b.idx);

        while (localSteps.length > 0) {
            const elem = localSteps[localSteps.length - 1];

            if (elem.kind == StepType.Repeat) {
                const laps = elem.reps!;
                const wrapped = localSteps
                    .filter(
                        (ws) => ws.idx >= elem.begin_idx! && ws.idx < elem.idx,
                    )
                    .map((e) => WorkoutUtils.workoutStepToLeafStep(e, fromKg));
                const element: StepGroup = {
                    laps,
                    wrapped,
                };
                elements.push(element);
                localSteps = localSteps.filter(
                    (ws) => ws.idx < elem.begin_idx!,
                );
            } else {
                elements.push(WorkoutUtils.workoutStepToLeafStep(elem, fromKg));
                localSteps.pop();
            }
        }

        return elements.reverse();
    }

    private static workoutStepToLeafStep(
        step: WorkoutStep,
        fromKg: (kg: number) => number,
    ): LeafStep {
        if (step.kind == StepType.Rest) {
            return {
                kind: StepType.Rest,
                time:
                    step.time !== null && step.time !== undefined
                        ? step.time / 1000
                        : null,
                reps:
                    step.reps !== null && step.reps !== undefined
                        ? step.reps
                        : null,
                ex_cat: null,
                ex_id: null,
                weight: null,
                notes: step.notes && step.notes!.length > 0 ? step.notes : "",
                exercise: null,
            };
        } else {
            return {
                kind: step.kind,
                // Reps take precedence over time in the editor, a leftover time would be hidden there
                time:
                    step.reps == null &&
                    step.time !== null &&
                    step.time !== undefined
                        ? step.time / 1000
                        : null,
                reps:
                    step.reps !== null && step.reps !== undefined
                        ? step.reps
                        : null,
                ex_cat: step.ex_cat,
                ex_id: step.ex_id,
                weight:
                    step.weight != null
                        ? Math.round(fromKg(step.weight) * 10) / 10
                        : null,
                notes: step.notes && step.notes!.length > 0 ? step.notes : "",
                exercise: null,
            };
        }
    }

    public static validateSteps(steps: WorkoutStepData[]): boolean {
        return (
            steps.length > 0 &&
            steps.every((step) =>
                WorkoutUtils.isStepGroup(step)
                    ? step.laps >= 1 &&
                      step.wrapped.length > 0 &&
                      step.wrapped.every(WorkoutUtils.validateLeaf)
                    : WorkoutUtils.validateLeaf(step),
            )
        );
    }

    // Leftover fields on rest steps are not checked here, they're dropped before sending to the backend
    public static leafErrors(leaf: LeafStep): LeafErrors {
        const isActive = leaf.kind != StepType.Rest;
        return {
            time: leaf.time != null && leaf.time <= 0,
            // Only plain exercises require one, warm-ups and the like may go without
            exercise:
                leaf.kind == StepType.Exercise &&
                (leaf.ex_cat == null || leaf.ex_id == null),
            weight: isActive && leaf.weight != null && leaf.weight < 0,
            reps: isActive && leaf.reps != null && leaf.reps < 1,
            notes: "",
        };
    }

    private static validateLeaf(leaf: LeafStep): boolean {
        return !Object.values(WorkoutUtils.leafErrors(leaf)).some(Boolean);
    }

    public static toWorkoutSteps(
        steps: WorkoutStepData[],
        workout: string,
        toKg: (value: number) => number,
    ): WorkoutStep[] {
        const result: WorkoutStep[] = [];

        for (const step of steps) {
            if (WorkoutUtils.isStepGroup(step)) {
                const begin = result.length;
                for (const leaf of step.wrapped) {
                    result.push(
                        WorkoutUtils.leafToWorkoutStep(
                            leaf,
                            workout,
                            result.length,
                            toKg,
                        ),
                    );
                }
                result.push({
                    workout,
                    idx: result.length,
                    kind: StepType.Repeat,
                    reps: step.laps,
                    begin_idx: begin,
                    ex_cat: null,
                    ex_id: null,
                    time: null,
                    weight: null,
                    notes: "",
                    exercise: null,
                });
            } else {
                result.push(
                    WorkoutUtils.leafToWorkoutStep(
                        step,
                        workout,
                        result.length,
                        toKg,
                    ),
                );
            }
        }

        return result;
    }

    private static leafToWorkoutStep(
        leaf: LeafStep,
        workout: string,
        idx: number,
        toKg: (value: number) => number,
    ): WorkoutStep {
        if (leaf.kind == StepType.Rest) {
            return {
                workout,
                idx: idx,
                kind: StepType.Rest,
                time: leaf.time != null ? leaf.time * 1000 : null,
                begin_idx: null,
                ex_cat: null,
                ex_id: null,
                reps: null,
                weight: null,
                notes:
                    leaf.notes !== null && leaf.notes.length > 0
                        ? leaf.notes
                        : null,
                exercise: null,
            };
        } else {
            return {
                workout,
                idx: idx,
                kind: leaf.kind,
                time: leaf.time != null ? leaf.time * 1000 : null,
                ex_cat: leaf.ex_cat,
                ex_id: leaf.ex_id,
                reps: leaf.reps,
                weight:
                    leaf.weight != null
                        ? leaf.weight > 0
                            ? Math.round(toKg(leaf.weight) * 10) / 10
                            : null
                        : null,
                begin_idx: null,
                notes:
                    leaf.notes !== null && leaf.notes.length > 0
                        ? leaf.notes
                        : null,
                exercise: null,
            };
        }
    }

    public static getDefaultLeaf(kind: StepType): LeafStep {
        switch (kind) {
            case StepType.Rest:
                return WorkoutUtils.DEFAULT_REST;
            case StepType.Cooldown:
                return WorkoutUtils.DEFAULT_COOLDOWN;
            case StepType.Exercise:
                return WorkoutUtils.DEFAULT_EXERCISE;
            case StepType.Recovery:
                return WorkoutUtils.DEFAULT_RECOVERY;
            case StepType.Warmup:
                return WorkoutUtils.DEFAULT_WARMUP;
            case StepType.Other:
            default:
                return WorkoutUtils.DEFAULT_OTHER;
        }
    }

    public static DEFAULT_REST: LeafStep = {
        kind: StepType.Rest,
        time: 120,
        ex_cat: null,
        ex_id: null,
        reps: null,
        weight: null,
        notes: "",
        exercise: null,
    };

    public static DEFAULT_EXERCISE: LeafStep = {
        kind: StepType.Exercise,
        time: null,
        ex_cat: null,
        ex_id: null,
        reps: 8,
        weight: 0,
        notes: "",
        exercise: null,
    };

    public static DEFAULT_WARMUP: LeafStep = {
        ...this.DEFAULT_EXERCISE,
        kind: StepType.Warmup,
    };

    public static DEFAULT_COOLDOWN: LeafStep = {
        ...this.DEFAULT_EXERCISE,
        kind: StepType.Cooldown,
    };

    public static DEFAULT_RECOVERY: LeafStep = {
        ...this.DEFAULT_EXERCISE,
        kind: StepType.Recovery,
    };

    public static DEFAULT_OTHER: LeafStep = {
        ...this.DEFAULT_EXERCISE,
        kind: StepType.Other,
    };

    public static DEFAULT_GROUP: StepGroup = {
        laps: 3,
        wrapped: [
            { ...WorkoutUtils.DEFAULT_EXERCISE },
            { ...WorkoutUtils.DEFAULT_REST },
        ],
    };
}
