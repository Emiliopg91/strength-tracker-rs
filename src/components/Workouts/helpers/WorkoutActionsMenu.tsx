import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { useLoadingTask } from "@/hooks/useLoadingTask";
import { BackendClient } from "@/utils/backend/client";
import { useContext, useState } from "react";
import {
    IconButton,
    ListItemText,
    Menu,
    MenuItem,
    MenuList,
    Paper,
    Popper,
    Tooltip,
} from "@mui/material";
import MenuIcon from "@mui/icons-material/Menu";
import ChevronRightIcon from "@mui/icons-material/ChevronRight";
import { DeviceContext } from "@/context/DeviceContext";

type Props = {
    workoutName: string;
    dense?: boolean;
    send: boolean;
    onOpenEdit: () => void;
    onOpenDuplicate?: () => void;
};

export function WorkoutActionsMenu({
    workoutName,
    send,
    onOpenEdit,
    onOpenDuplicate = undefined,
    dense = false,
}: Props) {
    const withLoading = useLoadingTask();
    const { translate } = useContext(I18nSettingsContext);
    const { devices } = useContext(DeviceContext);
    const [menuAnchor, setMenuAnchor] = useState<HTMLElement | null>(null);
    const [sendMenuAnchor, setSendMenuAnchor] = useState<HTMLElement | null>(
        null,
    );

    const closeMenus = () => {
        setSendMenuAnchor(null);
        setMenuAnchor(null);
    };

    const sendToDevice = (serial: string) => {
        withLoading(BackendClient.sendToDevice(workoutName, serial));
    };

    const sendToConnect = (serial: string) => {
        withLoading(BackendClient.sendToConnect(workoutName, serial));
    };

    const saveAsPdf = () => {
        withLoading(BackendClient.loadWorkoutPage(workoutName));
    };

    const workoutDevices = devices
        .filter((d) => {
            return d.strength;
        })
        .toSorted((a, b) => a.model.localeCompare(b.model));

    return (
        <>
            <Tooltip title={translate("actions")}>
                <IconButton
                    size={dense ? "small" : "medium"}
                    sx={dense ? { padding: 0 } : undefined}
                    onClick={(e) => setMenuAnchor(e.currentTarget)}
                >
                    <MenuIcon fontSize={dense ? "small" : "medium"} />
                </IconButton>
            </Tooltip>
            <Menu
                anchorEl={menuAnchor}
                open={Boolean(menuAnchor)}
                onClose={closeMenus}
                anchorOrigin={{ vertical: "bottom", horizontal: "right" }}
                transformOrigin={{ vertical: "top", horizontal: "right" }}
            >
                <MenuItem
                    onClick={() => {
                        closeMenus();
                        onOpenEdit();
                    }}
                >
                    <ListItemText>{translate("edit_workout")}</ListItemText>
                </MenuItem>
                <MenuItem
                    onClick={(e) => setSendMenuAnchor(e.currentTarget)}
                    onMouseEnter={(e) => setSendMenuAnchor(e.currentTarget)}
                    // The Popper is a React child, so hovering it doesn't count as leaving
                    onMouseLeave={() => setSendMenuAnchor(null)}
                    selected={Boolean(sendMenuAnchor)}
                    disabled={workoutDevices.length == 0 || !send}
                >
                    <ListItemText>{translate("send_to")}</ListItemText>
                    <ChevronRightIcon fontSize="small" />
                    <Popper
                        anchorEl={sendMenuAnchor}
                        open={Boolean(sendMenuAnchor)}
                        placement="left-start"
                        sx={{ zIndex: (theme) => theme.zIndex.modal + 1 }}
                    >
                        <Paper elevation={8}>
                            <MenuList>
                                {workoutDevices.map((device, idx) => {
                                    // Plugged in USB first, then Connect; otherwise send_to_device leaves it pending for the next plug
                                    const viaConnect =
                                        !device.usb && device.connect;
                                    return (
                                        <MenuItem
                                            key={"dev-" + idx}
                                            onClick={(e) => {
                                                e.stopPropagation();
                                                closeMenus();
                                                if (viaConnect) {
                                                    sendToConnect(
                                                        device.serial_number,
                                                    );
                                                } else {
                                                    sendToDevice(
                                                        device.serial_number,
                                                    );
                                                }
                                            }}
                                        >
                                            {device.manufacturer +
                                                " " +
                                                device.model +
                                                (viaConnect
                                                    ? " (Connect)"
                                                    : "")}
                                        </MenuItem>
                                    );
                                })}
                            </MenuList>
                        </Paper>
                    </Popper>
                </MenuItem>
                <MenuItem
                    onClick={() => {
                        closeMenus();
                        saveAsPdf();
                    }}
                >
                    <ListItemText>{translate("save_as_pdf")}</ListItemText>
                </MenuItem>
                {onOpenDuplicate && (
                    <MenuItem
                        onClick={() => {
                            closeMenus();
                            onOpenDuplicate();
                        }}
                    >
                        <ListItemText>{translate("duplicate")}</ListItemText>
                    </MenuItem>
                )}
            </Menu>
        </>
    );
}
