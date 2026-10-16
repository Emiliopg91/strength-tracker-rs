import ReactDOM from "react-dom/client";
import App from "@/components/App/App";
import { ThemeProvider } from "@mui/material/styles";
import CssBaseline from "@mui/material/CssBaseline";
import { theme } from "@/theme";

import "leaflet/dist/leaflet.css";
import L from "leaflet";
import markerIcon2x from "leaflet/dist/images/marker-icon-2x.png";
import markerIcon from "leaflet/dist/images/marker-icon.png";
import markerShadow from "leaflet/dist/images/marker-shadow.png";

import "@/styles/main.css";
import "@/styles/common.css";
import { AppProvider } from "./context/AppProvider";
import { I18nSettingsProvider } from "./context/I18nSettingsProvider";
import { LoadingProvider } from "./context/LoadingProvider";
import { DeviceProvider } from "./context/DeviceProvider";

delete (L.Icon.Default.prototype as unknown as { _getIconUrl?: unknown })
    ._getIconUrl;
L.Icon.Default.mergeOptions({
    iconRetinaUrl: markerIcon2x,
    iconUrl: markerIcon,
    shadowUrl: markerShadow,
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <ThemeProvider theme={theme}>
        <CssBaseline />
        <LoadingProvider>
            <I18nSettingsProvider>
                <AppProvider>
                    <DeviceProvider>
                        <App />
                    </DeviceProvider>
                </AppProvider>
            </I18nSettingsProvider>
        </LoadingProvider>
    </ThemeProvider>,
);
