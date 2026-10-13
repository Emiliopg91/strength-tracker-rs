import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { Backdrop, CircularProgress } from "@mui/material";
import { useContext, useEffect, useState } from "react";
import "@/styles/Loading/Loading.css";
import { LoadingStateContext } from "@/context/LoadingContext";
import { AppContext } from "@/context/AppContext";

// Short tasks (e.g. fetches when opening a modal) would otherwise flash the dark overlay
const SHOW_DELAY_MS = 150;

export function Loading() {
  const { translate } = useContext(I18nSettingsContext);
  const { appReady } = useContext(AppContext);

  const loading = useContext(LoadingStateContext);
  const [delayElapsed, setDelayElapsed] = useState(false);

  useEffect(() => {
    if (!loading) {
      setDelayElapsed(false);
      return;
    }
    const timeout = setTimeout(() => setDelayElapsed(true), SHOW_DELAY_MS);
    return () => clearTimeout(timeout);
  }, [loading]);

  if (appReady && !loading) {
    return null;
  }
  // While the delay runs the backdrop stays mounted but transparent, so it still blocks input
  const hidden = appReady && !delayElapsed;
  return (
    <Backdrop
      open={true}
      transitionDuration={0}
      className={
        "loading-backdrop" + (hidden ? " loading-backdrop-hidden" : "")
      }
    >
      {!hidden && (
        <CircularProgress aria-label={translate("loading")} size="5rem" />
      )}
    </Backdrop>
  );
}
