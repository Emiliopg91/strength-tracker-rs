import ReactDOM from "react-dom/client";

import "@/styles/Report/Report.css";
import { I18nSettingsProvider } from "./context/I18nSettingsProvider";
import { AppProvider } from "./context/AppProvider";
import WorkoutPrintView from "./components/Workouts/views/WorkoutPrintView";

const params = new URLSearchParams(window.location.search);
// Id of the workout; the view titles the window with its name once loaded
const workout = Number(params.get("workout"));

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <I18nSettingsProvider>
        <AppProvider notifyBackend={false}>
            <>
                <WorkoutPrintView workout={workout} />
            </>
        </AppProvider>
    </I18nSettingsProvider>,
);
