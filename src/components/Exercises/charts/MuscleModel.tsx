import { JSX } from "react";
import Body, { ExtendedBodyPart, Slug } from "@mjcdev/react-body-highlighter";

type Props = {
    primary: string[];
    secondary: string[];
};

export function MuscleModel({
    primary: primary_input,
    secondary: secondary_input,
}: Props): JSX.Element {
    const intensities = new Map<Slug, number>();
    for (const slug of exercisesToSlugs(secondary_input)) {
        intensities.set(slug, SECONDARY_INTENSITY);
    }
    // Set last so a muscle both primary and secondary shows as primary
    for (const slug of exercisesToSlugs(primary_input)) {
        intensities.set(slug, PRIMARY_INTENSITY);
    }
    const data: ExtendedBodyPart[] = [...intensities].map(
        ([slug, intensity]) => ({ slug, intensity }),
    );

    // The library has no prop to disable the pointer cursor and click handling of its paths
    return (
        <>
            <div style={MODEL_STYLE}>
                <Body data={data} colors={COLORS} scale={SCALE} side="front" />
            </div>
            <div style={MODEL_STYLE}>
                <Body data={data} colors={COLORS} scale={SCALE} side="back" />
            </div>
        </>
    );
}

// Each model is 200 * SCALE px wide and keeps its 1:2 ratio
const SCALE = 0.575;
// Takes out the empty side margins of the model, leaving 100px of width
const MODEL_STYLE = { pointerEvents: "none", margin: "0 -7px" } as const;
// Indexed by intensity - 1
const COLORS = ["yellow", "red"];
const SECONDARY_INTENSITY = 1;
const PRIMARY_INTENSITY = 2;

// The model has no abductors region: the glutes are the closest hip abductors it shows
const MUSCLE_IDENTIFIERS: Record<string, Slug[]> = {
    abductors: ["gluteal"],
    abs: ["abs"],
    adductors: ["adductors"],
    biceps: ["biceps"],
    calves: ["calves"],
    chest: ["chest"],
    forearm: ["forearm"],
    glutes: ["gluteal"],
    hamstrings: ["hamstring"],
    hips: ["gluteal", "adductors"],
    lats: ["upper-back"],
    lower_back: ["lower-back"],
    obliques: ["obliques"],
    quads: ["quadriceps"],
    shoulders: ["deltoids"],
    traps: ["trapezius"],
    triceps: ["triceps"],
};

function exercisesToSlugs(exercises: string[]): Slug[] {
    return exercises.flatMap((muscle) => MUSCLE_IDENTIFIERS[muscle] ?? []);
}
