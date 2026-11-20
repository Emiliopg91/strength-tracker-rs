import { AppContext } from "@/context/AppContext";
import { BackendClient } from "@/utils/backend/client";
import { ExerciseListItem, WorkoutDetails } from "@/utils/backend/models";
import { JSX, useContext, useEffect, useState } from "react";
import { WorkoutStructure } from "./WorkoutStructure";

import "@/styles/Workouts/WorkoutPrintView.css";

type Props = {
    /** Id of the workout to print. */
    workout: number;
};

export default function WorkoutPrintView({
    workout: workoutId,
}: Props): JSX.Element {
    const { appReady } = useContext(AppContext);
    const [workout, setWorkout] = useState<WorkoutDetails>();
    const [exercises, setExercises] = useState<ExerciseListItem[]>();

    useEffect(() => {
        if (!appReady) return;
        BackendClient.getWorkoutDetails(workoutId).then((details) => {
            document.title = details.name;
            setWorkout(details);
        });
        BackendClient.getExercises().then(setExercises);
    }, [appReady, workoutId]);

    useEffect(() => {
        if (!workout || !exercises) return;
        (async () => {
            await document.fonts.ready;
            await BackendClient.workoutPrintReady(workout.name);
        })();
    }, [workout, exercises]);

    if (!workout || !exercises) {
        return <></>;
    }

    return (
        <div className="workout-print">
            <h1>{workout.name}</h1>
            <WorkoutStructure workout={workout} exercises={exercises} />
        </div>
    );
}
