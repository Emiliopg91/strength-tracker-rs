import { createContext } from "react";

interface LoadingContextType {
    startLoading: () => void;
    finishLoading: () => void;
}

const defaultValue: LoadingContextType = {
    startLoading: () => {
        /* empty */
    },
    finishLoading: () => {
        /* empty */
    },
};

// The loading flag, apart from LoadingContext so toggling it only re-renders the components that read it
export const LoadingStateContext = createContext(false);

// Actions only: their value never changes, so components that just trigger loading don't re-render
export const LoadingContext = createContext(defaultValue);
