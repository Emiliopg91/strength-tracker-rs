import { JSX } from "react";
import Model, {
    IExerciseData,
    IModelProps,
    Muscle,
} from "react-body-highlighter";

type Props = {
    primary: string[];
    secondary: string[];
};

export function MuscleModel({
    primary: primary_input,
    secondary: secondary_input,
}: Props): JSX.Element {
    const data: IExerciseData[] = [
        { name: "Primary1", muscles: exercisesToIdentifiers(primary_input) },
        { name: "Primary2", muscles: exercisesToIdentifiers(primary_input) },
        { name: "Secondary", muscles: exercisesToIdentifiers(secondary_input) },
    ];
    const modelProps: IModelProps = {
        data,
        style: { width: "5rem" },
        svgStyle: { pointerEvents: "none" },
        highlightedColors: ["yellow", "red"],
    };

    return (
        <>
            <Model {...modelProps} type="anterior" />
            <Model {...modelProps} type="posterior" />
        </>
    );
}

const MUSCLE_IDENTIFIERS: Record<string, string[]> = {
    abductors: ["abductors"],
    abs: ["abs"],
    adductors: ["adductor"],
    biceps: ["biceps"],
    calves: ["calves"],
    chest: ["chest"],
    forearm: ["forearm"],
    glutes: ["gluteal"],
    hamstrings: ["hamstring"],
    hips: ["abductors", "adductor"],
    lats: ["upper-back"],
    lower_back: ["lower-back"],
    obliques: ["obliques"],
    quads: ["quadriceps"],
    shoulders: ["front-deltoids", "back-deltoids"],
    traps: ["neck", "trapezius"],
    triceps: ["triceps"],
};

function exercisesToIdentifiers(exercises: string[]): Muscle[] {
    const identifiers = new Set<Muscle>();
    for (const muscle of exercises) {
        for (const identifier of MUSCLE_IDENTIFIERS[muscle] ?? []) {
            identifiers.add(identifier as Muscle);
        }
    }

    return [...identifiers];
}
