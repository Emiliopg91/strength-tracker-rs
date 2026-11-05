import { describe, expect, it } from "vitest";
import { StepType, WorkoutStep } from "@/utils/backend/models";
import { WorkoutUtils } from "@/utils/WorkoutUtils";

const exerciseStep = (overrides: Partial<WorkoutStep>): WorkoutStep => ({
    workout: "w",
    idx: 0,
    kind: StepType.Exercise,
    begin_idx: null,
    ex_cat: 7,
    ex_id: 16,
    reps: null,
    time: null,
    weight: 16,
    notes: null,
    exercise: null,
    ...overrides,
});

const parse = (step: WorkoutStep) =>
    WorkoutUtils.parseWorkoutSteps([step], (kg) => kg);

describe("WorkoutUtils", () => {
    it("only requires an exercise on plain exercise steps", () => {
        const noExercise = { ex_cat: null, ex_id: null };

        for (const kind of [
            StepType.Warmup,
            StepType.Cooldown,
            StepType.Recovery,
            StepType.Other,
        ]) {
            const steps = parse(exerciseStep({ ...noExercise, kind }));
            expect(steps[0]).toMatchObject({ kind });
            expect(WorkoutUtils.validateSteps(steps)).toBe(true);
        }

        const steps = parse(exerciseStep(noExercise));
        expect(WorkoutUtils.validateSteps(steps)).toBe(false);
    });

    it("drops a leftover zero time on rep-based exercises so they stay valid", () => {
        const steps = parse(exerciseStep({ reps: 30, time: 0 }));

        expect(steps[0]).toMatchObject({ reps: 30, time: null });
        expect(WorkoutUtils.validateSteps(steps)).toBe(true);
    });

    it("keeps the time, in seconds, on time-based exercises", () => {
        const steps = parse(exerciseStep({ time: 60 }));

        expect(steps[0]).toMatchObject({ reps: null, time: 60 });
        expect(WorkoutUtils.validateSteps(steps)).toBe(true);
    });

    it("rejects an exercise with no exercise selected", () => {
        const steps = [structuredClone(WorkoutUtils.DEFAULT_GROUP)];

        expect(WorkoutUtils.validateSteps(steps)).toBe(false);
    });
});
