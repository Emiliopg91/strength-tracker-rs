import { AppEnvironment } from "@/utils/backend/models";
import { createContext } from "react";

export type ExerciseOption = {
    label: string;
    ex_cat: number;
    ex_id: number;
    notes: string | null;
};

interface AppContexType {
    appReady: boolean;
    environment: AppEnvironment;
    sessionsVersion: number;
    // Empty until loaded, or if loading failed
    exercisesCatalog: ExerciseOption[];
    updateExerciseNotes: (ex_cat: number, ex_id: number, notes: string) => void;
    // Translated names, keyed by the Garmin names of the catalog; ids until it loads
    exerciseName: (ex_cat: number | null, ex_id: number | null) => string;
    categoryName: (ex_cat: number) => string;
    connectUser: string | null;
    progressionsAvailable: boolean;
    setProgressionsAvailable: () => void;
    unsetProgressionsAvailable: () => void;
}

const defaultValue: AppContexType = {
    appReady: false,
    environment: AppEnvironment.Release,
    sessionsVersion: 0,
    exercisesCatalog: [],
    updateExerciseNotes: () => {
        /** */
    },
    exerciseName: (ex_cat, ex_id) =>
        ex_cat == null || ex_id == null ? "" : `${ex_cat}/${ex_id}`,
    categoryName: (ex_cat) => `${ex_cat}`,
    connectUser: null,
    progressionsAvailable: false,
    setProgressionsAvailable: () => {
        /** */
    },
    unsetProgressionsAvailable: () => {
        /** */
    },
};

export const AppContext = createContext(defaultValue);
