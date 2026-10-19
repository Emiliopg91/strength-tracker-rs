import ReactDOM from "react-dom/client";

import "@/styles/Report/Report.css";
import { I18nSettingsProvider } from "./context/I18nSettingsProvider";
import { AppProvider } from "./context/AppProvider";
import Report from "./components/Report/Report";

const params = new URLSearchParams(window.location.search);
const from = Number(params.get("from"));
const to = Number(params.get("to"));

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <I18nSettingsProvider>
        <AppProvider notifyBackend={false}>
            <>
                <Report beginDate={from} endDate={to} />
            </>
        </AppProvider>
    </I18nSettingsProvider>,
);
