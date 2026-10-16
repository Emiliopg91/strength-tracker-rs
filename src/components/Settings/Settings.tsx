import { AppContext } from "@/context/AppContext";
import { useLoadingTask } from "@/hooks/useLoadingTask";
import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { BackendClient } from "@/utils/backend/client";
import {
    AppEnvironment,
    DistanceUnit,
    Languages,
    WeightUnit,
} from "@/utils/backend/models";
import { useContext, useState } from "react";
import {
    Button,
    MenuItem,
    Select,
    Dialog,
    DialogContent,
    DialogTitle,
    IconButton,
} from "@mui/material";
import CloseIcon from "@mui/icons-material/Close";
import { UploadToCloudMenu } from "./helpers/UploadToCloudMenu";

type Props = {
    onClose: () => void;
};

export function Settings({ onClose }: Props) {
    const { environment } = useContext(AppContext);
    const withLoading = useLoadingTask();
    const { settings, updateSettings, translate, refreshTranslations } =
        useContext(I18nSettingsContext);

    const [weightUnit, setWeightUnit] = useState(settings.weight_unit);
    const [distanceUnit, setDistanceUnit] = useState(settings.distance_unit);
    const [startOnBoot, setStartOnBoot] = useState(settings.start_boot);
    const [language, setLanguage] = useState(settings.language);
    const [onDeviceConnect, setOnDeviceConnect] = useState(
        settings.on_device_connect,
    );
    const [startIntoTray, setStartIntoTray] = useState(
        settings.start_into_tray,
    );
    const [closeToTray, setCloseToTray] = useState(settings.close_to_tray);

    const updateWeightUnit = (value: WeightUnit) => {
        withLoading(
            BackendClient.updateSettingsValue("weight_unit", value),
        ).then(() => {
            setWeightUnit(value);
            updateSettings({ weight_unit: value });
        });
    };

    const updateDistanceUnit = (value: DistanceUnit) => {
        withLoading(
            BackendClient.updateSettingsValue("distance_unit", value),
        ).then(() => {
            setDistanceUnit(value);
            updateSettings({ distance_unit: value });
        });
    };

    const updateStartOnBoot = (value: boolean) => {
        withLoading(
            BackendClient.updateSettingsValue(
                "start_boot",
                value ? "true" : "false",
            ),
        ).then(() => {
            setStartOnBoot(value);
            updateSettings({ start_boot: value });
        });
    };

    const updateLanguage = (value: Languages) => {
        withLoading(BackendClient.updateSettingsValue("language", value)).then(
            () => {
                setLanguage(value);
                updateSettings({ language: value });
                refreshTranslations();
            },
        );
    };

    const updateOnDeviceConnect = (value: boolean) => {
        withLoading(
            BackendClient.updateSettingsValue(
                "on_device_connect",
                value ? "true" : "false",
            ),
        ).then(() => {
            setOnDeviceConnect(value);
            updateSettings({ on_device_connect: value });
        });
    };

    const updateCloseToTray = (value: boolean) => {
        withLoading(
            BackendClient.updateSettingsValue(
                "close_to_tray",
                value ? "true" : "false",
            ),
        ).then(() => {
            setCloseToTray(value);
            updateSettings({ close_to_tray: value });
        });
    };

    const updateStartIntoTray = (value: boolean) => {
        withLoading(
            BackendClient.updateSettingsValue(
                "start_into_tray",
                value ? "true" : "false",
            ),
        ).then(() => {
            setStartIntoTray(value);
            updateSettings({ start_into_tray: value });
        });
    };

    const exportDatabase = () => {
        withLoading(BackendClient.exportDatabase());
    };

    return (
        <>
            <Dialog open={true} onClose={onClose}>
                <DialogTitle>
                    {translate("settings")}
                    <IconButton
                        onClick={onClose}
                        className="modal-close-button"
                    >
                        <CloseIcon />
                    </IconButton>
                </DialogTitle>

                <DialogContent dividers>
                    <fieldset>
                        <legend>{translate("user_settings")}</legend>
                        <table>
                            <colgroup>
                                <col className="col-250"></col>
                                <col className="col-200"></col>
                            </colgroup>
                            <tbody>
                                <tr>
                                    <td>{translate("language")}</td>
                                    <td>
                                        <Select
                                            size="small"
                                            fullWidth
                                            value={language}
                                            onChange={(e) =>
                                                updateLanguage(
                                                    e.target.value as Languages,
                                                )
                                            }
                                        >
                                            <MenuItem value={Languages.English}>
                                                {translate("lang_en")}
                                            </MenuItem>
                                            <MenuItem value={Languages.Spanish}>
                                                {translate("lang_es")}
                                            </MenuItem>
                                        </Select>
                                    </td>
                                </tr>
                                <tr>
                                    <td>{translate("weight_unit")}</td>
                                    <td>
                                        <Select
                                            size="small"
                                            fullWidth
                                            value={weightUnit}
                                            onChange={(e) =>
                                                updateWeightUnit(
                                                    e.target
                                                        .value as WeightUnit,
                                                )
                                            }
                                        >
                                            <MenuItem
                                                value={WeightUnit.Kilograms}
                                            >
                                                {translate(
                                                    "weight_unit_kilograms",
                                                )}
                                            </MenuItem>
                                            <MenuItem value={WeightUnit.Pounds}>
                                                {translate(
                                                    "weight_unit_pounds",
                                                )}
                                            </MenuItem>
                                        </Select>
                                    </td>
                                </tr>
                                <tr>
                                    <td>{translate("distance_unit")}</td>
                                    <td>
                                        <Select
                                            size="small"
                                            fullWidth
                                            value={distanceUnit}
                                            onChange={(e) =>
                                                updateDistanceUnit(
                                                    e.target
                                                        .value as DistanceUnit,
                                                )
                                            }
                                        >
                                            <MenuItem
                                                value={DistanceUnit.Kilometers}
                                            >
                                                {translate(
                                                    "distance_unit_kilometers",
                                                )}
                                            </MenuItem>
                                            <MenuItem
                                                value={DistanceUnit.Miles}
                                            >
                                                {translate(
                                                    "distance_unit_miles",
                                                )}
                                            </MenuItem>
                                        </Select>
                                    </td>
                                </tr>
                            </tbody>
                        </table>
                    </fieldset>
                    <br />
                    <fieldset>
                        <legend>{translate("application")}</legend>
                        <table>
                            <colgroup>
                                <col className="col-250"></col>
                                <col className="col-200"></col>
                            </colgroup>
                            <tbody>
                                <tr>
                                    <td>{translate("start_on_boot")}</td>
                                    <td>
                                        <Select
                                            size="small"
                                            fullWidth
                                            value={
                                                startOnBoot ? "true" : "false"
                                            }
                                            onChange={(e) =>
                                                updateStartOnBoot(
                                                    e.target.value === "true",
                                                )
                                            }
                                            disabled={
                                                environment ==
                                                AppEnvironment.Debug
                                            }
                                        >
                                            <MenuItem value="false">
                                                {translate(
                                                    "start_on_boot_false",
                                                )}
                                            </MenuItem>
                                            <MenuItem value="true">
                                                {translate(
                                                    "start_on_boot_true",
                                                )}
                                            </MenuItem>
                                        </Select>
                                    </td>
                                </tr>
                                <tr>
                                    <td>{translate("start_into_tray")}</td>
                                    <td>
                                        <Select
                                            size="small"
                                            fullWidth
                                            value={
                                                startIntoTray ? "true" : "false"
                                            }
                                            onChange={(e) =>
                                                updateStartIntoTray(
                                                    e.target.value === "true",
                                                )
                                            }
                                        >
                                            <MenuItem value="false">
                                                {translate("no")}
                                            </MenuItem>
                                            <MenuItem value="true">
                                                {translate("yes")}
                                            </MenuItem>
                                        </Select>
                                    </td>
                                </tr>
                                <tr>
                                    <td>{translate("close_to_tray")}</td>
                                    <td>
                                        <Select
                                            size="small"
                                            fullWidth
                                            value={
                                                closeToTray ? "true" : "false"
                                            }
                                            onChange={(e) =>
                                                updateCloseToTray(
                                                    e.target.value === "true",
                                                )
                                            }
                                        >
                                            <MenuItem value="false">
                                                {translate("no")}
                                            </MenuItem>
                                            <MenuItem value="true">
                                                {translate("yes")}
                                            </MenuItem>
                                        </Select>
                                    </td>
                                </tr>
                                <tr>
                                    <td>{translate("on_device_connect")}</td>
                                    <td>
                                        <Select
                                            size="small"
                                            fullWidth
                                            value={
                                                onDeviceConnect
                                                    ? "true"
                                                    : "false"
                                            }
                                            onChange={(e) =>
                                                updateOnDeviceConnect(
                                                    e.target.value === "true",
                                                )
                                            }
                                        >
                                            <MenuItem value="false">
                                                {translate("do_nothing")}
                                            </MenuItem>
                                            <MenuItem value="true">
                                                {translate(
                                                    "start_automatically",
                                                )}
                                            </MenuItem>
                                        </Select>
                                    </td>
                                </tr>
                                <tr>
                                    <td>{translate("database_operations")}</td>
                                    <td>
                                        <table style={{ width: "100%" }}>
                                            <tbody>
                                                <tr>
                                                    <td>
                                                        <Button
                                                            id="add-measure-button"
                                                            variant="contained"
                                                            className="full-width-button"
                                                            onClick={
                                                                exportDatabase
                                                            }
                                                        >
                                                            {translate(
                                                                "backup_database",
                                                            )}
                                                        </Button>
                                                    </td>
                                                </tr>
                                                <tr>
                                                    <td>
                                                        <UploadToCloudMenu />
                                                    </td>
                                                </tr>
                                            </tbody>
                                        </table>
                                    </td>
                                </tr>
                            </tbody>
                        </table>
                    </fieldset>
                </DialogContent>
            </Dialog>
        </>
    );
}
