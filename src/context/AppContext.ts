import { AppEnvironment } from "@/utils/backend/models";
import { createContext } from "react";

export type ExerciseOption = {
    label: string;
    ex_cat: number;
    ex_id: number;
};

interface AppContexType {
    appReady: boolean;
    environment: AppEnvironment;
    sessionsVersion: number;
    // Empty until loaded, or if loading failed
    exercisesCatalog: ExerciseOption[];
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
