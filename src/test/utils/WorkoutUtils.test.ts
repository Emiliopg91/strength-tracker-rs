import { describe, expect, it } from "vitest";
import { StepType, WorkoutStep } from "@/utils/backend/models";
import { WorkoutUtils } from "@/utils/WorkoutUtils";

const exerciseStep = (overrides: Partial<WorkoutStep>): WorkoutStep => ({
    workout: 1,
    idx: 0,
    kind: StepType.Exercise,
    begin_idx: null,
    ex_cat: 7,
    ex_id: 16,
    reps: null,
    time: null,
    weight: 16,
    one_rm_pct: null,
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

    it("round-trips nested repeats and root-level steps", () => {
        // warmup, then 3x [ex, 2x [ex, rest] ]
        const steps: WorkoutStep[] = [
            exerciseStep({ idx: 0, kind: StepType.Warmup }),
            exerciseStep({ idx: 1, ex_id: 1 }),
            exerciseStep({ idx: 2, ex_id: 2 }),
            exerciseStep({
                idx: 3,
                kind: StepType.Rest,
                ex_cat: null,
                ex_id: null,
                weight: null,
                time: 60,
            }),
            exerciseStep({
                idx: 4,
                kind: StepType.Repeat,
                begin_idx: 2,
                reps: 2,
                ex_cat: null,
                ex_id: null,
                weight: null,
            }),
            exerciseStep({
                idx: 5,
                kind: StepType.Repeat,
                begin_idx: 1,
                reps: 3,
                ex_cat: null,
                ex_id: null,
                weight: null,
            }),
        ];

        const parsed = WorkoutUtils.parseWorkoutSteps(steps, (kg) => kg);

        expect(parsed).toHaveLength(2);
        expect(parsed[0]).toMatchObject({ kind: StepType.Warmup });
        expect(parsed[1]).toMatchObject({
            laps: 3,
            wrapped: [
                { kind: StepType.Exercise, ex_id: 1 },
                {
                    laps: 2,
                    wrapped: [
                        { kind: StepType.Exercise, ex_id: 2 },
                        { kind: StepType.Rest, time: 60 },
                    ],
                },
            ],
        });
        expect(WorkoutUtils.validateSteps(parsed)).toBe(true);

        const written = WorkoutUtils.toWorkoutSteps(parsed, 1, (kg) => kg);
        expect(
            written.map(({ idx, kind, begin_idx, reps }) => ({
                idx,
                kind,
                begin_idx,
                reps,
            })),
        ).toEqual(
            steps.map(({ idx, kind, begin_idx, reps }) => ({
                idx,
                kind,
                begin_idx,
                reps,
            })),
        );
    });

    it("sends percentage loads without a weight, for the backend to resolve", () => {
        const steps = parse(
            exerciseStep({ reps: 5, weight: 84, one_rm_pct: 70 }),
        );

        expect(steps[0]).toMatchObject({ one_rm_pct: 70, weight: 84 });
        expect(WorkoutUtils.validateSteps(steps)).toBe(true);

        const [written] = WorkoutUtils.toWorkoutSteps(steps, 1, (kg) => kg);
        expect(written).toMatchObject({ one_rm_pct: 70, weight: null });
    });

    it("rounds percentage loads to the nearest kg like the backend", () => {
        expect(WorkoutUtils.percentLoad(120, 70)).toBe(84);
        expect(WorkoutUtils.percentLoad(101, 70)).toBe(71);
        expect(WorkoutUtils.percentLoad(101, 50)).toBe(51);
        expect(WorkoutUtils.percentLoad(103, 30)).toBe(31);
    });

    it("rejects percentages outside 1-100 or with decimals", () => {
        for (const one_rm_pct of [0, 101, 72.5]) {
            const steps = parse(exerciseStep({ reps: 5, one_rm_pct }));
            expect(WorkoutUtils.validateSteps(steps)).toBe(false);
        }
    });

    it("rejects a nested repeat with an invalid step inside", () => {
        const group = structuredClone(WorkoutUtils.DEFAULT_GROUP);
        const steps = [{ laps: 2, wrapped: [group] }];

        expect(WorkoutUtils.validateSteps(steps)).toBe(false);
    });
});
