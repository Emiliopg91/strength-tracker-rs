import { AppContext } from "@/context/AppContext";
import { BackendClient } from "@/utils/backend/client";
import { ExerciseListItem, WorkoutDetails } from "@/utils/backend/models";
import { JSX, useContext, useEffect, useState } from "react";
import { WorkoutStructure } from "./WorkoutStructure";

import "@/styles/Workouts/WorkoutPrintView.css";

type Props = {
    workout: string;
};

export default function WorkoutPrintView({
    workout: workout_name,
}: Props): JSX.Element {
    const { appReady } = useContext(AppContext);
    const [workout, setWorkout] = useState<WorkoutDetails>();
    const [exercises, setExercises] = useState<ExerciseListItem[]>();

    useEffect(() => {
        if (!appReady) return;
        BackendClient.getWorkoutDetails(workout_name).then(setWorkout);
        BackendClient.getExercises().then(setExercises);
    }, [appReady, workout_name]);

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
