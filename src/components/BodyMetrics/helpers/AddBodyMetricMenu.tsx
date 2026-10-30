import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { useLoadingTask } from "@/hooks/useLoadingTask";
import { BackendClient } from "@/utils/backend/client";
import { useContext, useState } from "react";
import { Button, Menu, MenuItem } from "@mui/material";
import { AppContext } from "@/context/AppContext";

type Props = {
    onAddManually: () => void;
    onImported: () => void;
};

export function AddBodyMetricMenu({ onAddManually, onImported }: Props) {
    const withLoading = useLoadingTask();
    const { translate } = useContext(I18nSettingsContext);
    const { connectUser } = useContext(AppContext);
    const [menuAnchor, setMenuAnchor] = useState<{
        top: number;
        left: number;
    } | null>(null);

    const importFromGarminConnect = () => {
        withLoading(BackendClient.importWeightFromConnect()).then(() => {
            onImported();
        });
    };

    return (
        <>
            <Button
                id="add-measure-button"
                variant="contained"
                className="full-width-button"
                onClick={(e) =>
                    connectUser
                        ? setMenuAnchor({
                              top: e.clientY,
                              left: e.clientX,
                          })
                        : onAddManually()
                }
            >
                {translate("add_entry")}
            </Button>
            <Menu
                id="add-measure-menu"
                anchorReference="anchorPosition"
                anchorPosition={menuAnchor ?? undefined}
                open={Boolean(menuAnchor)}
                onClose={() => setMenuAnchor(null)}
                anchorOrigin={{ vertical: "top", horizontal: "left" }}
                transformOrigin={{
                    vertical: "bottom",
                    horizontal: "left",
                }}
            >
                <MenuItem
                    onClick={() => {
                        setMenuAnchor(null);
                        onAddManually();
                    }}
                >
                    {translate("add_manually")}
                </MenuItem>
                <MenuItem
                    onClick={() => {
                        setMenuAnchor(null);
                        importFromGarminConnect();
                    }}
                >
                    {translate("import_garmin_connect")}
                </MenuItem>
            </Menu>
        </>
    );
}
