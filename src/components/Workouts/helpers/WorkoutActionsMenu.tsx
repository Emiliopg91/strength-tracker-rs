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
    onOpenEdit: () => void;
    onOpenDuplicate?: () => void;
    dense?: boolean;
};

export function WorkoutActionsMenu({
    workoutName,
    onOpenEdit,
    onOpenDuplicate = undefined,
    dense = false,
}: Props) {
    const withLoading = useLoadingTask();
    const { translate } = useContext(I18nSettingsContext);
    const { registeredDevices } = useContext(DeviceContext);
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
                    disabled={registeredDevices.length == 0}
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
                                {registeredDevices.map((device, idx) => (
                                    <MenuItem
                                        key={"dev-" + idx}
                                        onClick={(e) => {
                                            e.stopPropagation();
                                            closeMenus();
                                            sendToDevice(device.serial_number);
                                        }}
                                    >
                                        {device.manufacturer +
                                            " " +
                                            device.model}
                                    </MenuItem>
                                ))}
                            </MenuList>
                        </Paper>
                    </Popper>
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
