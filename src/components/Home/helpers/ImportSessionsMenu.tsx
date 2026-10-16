import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { useLoadingTask } from "@/hooks/useLoadingTask";
import { BackendClient } from "@/utils/backend/client";
import { useContext, useState } from "react";
import { Button, Menu, MenuItem } from "@mui/material";
import { DeviceContext } from "@/context/DeviceContext";

type Props = {
    onImported: () => void;
};

export function ImportSessionsMenu({ onImported }: Props) {
    const { availableDevices } = useContext(DeviceContext);
    const withLoading = useLoadingTask();
    const { translate } = useContext(I18nSettingsContext);
    const [importMenuAnchor, setImportMenuAnchor] = useState<{
        top: number;
        left: number;
    } | null>(null);

    const importFromDisk = () => {
        withLoading(BackendClient.importFromFiles()).then((count) => {
            if (count > 0) {
                onImported();
            }
        });
    };

    const importDevice = (serial: string) => {
        withLoading(BackendClient.importFromDevice(serial)).then((count) => {
            if (count > 0) {
                onImported();
            }
        });
    };

    return (
        <div className="list-action-bar">
            {availableDevices.length == 0 && (
                <Button
                    id="import-file-toggle"
                    variant="contained"
                    className="full-width-button"
                    onClick={importFromDisk}
                >
                    {translate("import_from_disk")}
                </Button>
            )}
            {availableDevices.length > 0 && (
                <>
                    <Button
                        id="import-file-toggle"
                        variant="contained"
                        className="full-width-button"
                        onClick={(e) =>
                            setImportMenuAnchor({
                                top: e.clientY,
                                left: e.clientX,
                            })
                        }
                    >
                        {translate("import_sessions")}
                    </Button>
                    <Menu
                        id="import-file-menu"
                        anchorReference="anchorPosition"
                        anchorPosition={importMenuAnchor ?? undefined}
                        open={Boolean(importMenuAnchor)}
                        onClose={() => setImportMenuAnchor(null)}
                        anchorOrigin={{ vertical: "top", horizontal: "left" }}
                        transformOrigin={{
                            vertical: "bottom",
                            horizontal: "left",
                        }}
                    >
                        <MenuItem
                            onClick={() => {
                                setImportMenuAnchor(null);
                                importFromDisk();
                            }}
                        >
                            {translate("import_from_disk")}
                        </MenuItem>
                        {availableDevices.map((device, idx) => (
                            <MenuItem
                                key={"dev-" + idx}
                                onClick={() => {
                                    setImportMenuAnchor(null);
                                    importDevice(device.serial_number);
                                }}
                            >
                                {translate("import_from_device", [
                                    device.manufacturer + " " + device.model,
                                ])}
                            </MenuItem>
                        ))}
                    </Menu>
                </>
            )}
        </div>
    );
}
