import ReactDOM from "react-dom/client";

import "@/styles/Report/Report.css";
import { I18nSettingsProvider } from "./context/I18nSettingsProvider";
import { AppProvider } from "./context/AppProvider";
import WorkoutPrintView from "./components/Workouts/WorkoutPrintView";

const params = new URLSearchParams(window.location.search);
const workout = params.get("workout")!;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <I18nSettingsProvider>
        <AppProvider notifyBackend={false}>
            <>
                <WorkoutPrintView workout={workout} />
            </>
        </AppProvider>
    </I18nSettingsProvider>,
);
