import { useLoadingTask } from "@/hooks/useLoadingTask";
import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { BackendClient } from "@/utils/backend/client";
import { BodyMetricListItem } from "@/utils/backend/models";
import { useContext } from "react";
import {
    Button,
    Dialog,
    DialogContent,
    DialogTitle,
    IconButton,
} from "@mui/material";
import CloseIcon from "@mui/icons-material/Close";

type Props = {
    measures: BodyMetricListItem;
    onClose: () => void;
    onDelete: () => void;
};

export function BodyMetricsDetailsModal({
    measures,
    onClose,
    onDelete,
}: Props) {
    const withLoading = useLoadingTask();
    const {
        translate,
        fromKg,
        getWeightUnit,
        formatDate,
        formatNumber,
        formatPercent,
    } = useContext(I18nSettingsContext);

    const deleteEntry = () => {
        withLoading(BackendClient.deleteBodyMetric(measures.date)).then(() => {
            onDelete();
            onClose();
        });
    };

    return (
        <Dialog open={true} onClose={onClose}>
            <DialogTitle>
                {formatDate(measures.date)}
                <IconButton onClick={onClose} className="modal-close-button">
                    <CloseIcon />
                </IconButton>
            </DialogTitle>

            <DialogContent dividers>
                <table id="workout-details-table">
                    <colgroup>
                        <col className="col-200" />
                        <col className="col-150" />
                        <col />
                    </colgroup>
                    <tbody>
                        <tr>
                            <td>{translate("weight")}:</td>
                            <td>
                                {formatNumber(fromKg(measures.weight), 1)}{" "}
                                {getWeightUnit()}
                            </td>
                        </tr>
                        <tr>
                            <td>{translate("fat_ratio")}:</td>
                            <td>
                                {formatPercent(measures.fat_ratio / 100, 1)}
                            </td>
                        </tr>
                        <tr>
                            <td>{translate("fat_mass")}:</td>
                            <td>
                                {formatNumber(
                                    fromKg(
                                        measures.weight *
                                            (measures.fat_ratio / 100),
                                    ),
                                    1,
                                )}{" "}
                                {getWeightUnit()}
                            </td>
                        </tr>
                        <tr>
                            <td>{translate("lean_mass")}:</td>
                            <td>
                                {formatNumber(fromKg(measures.lean_mass), 1)}{" "}
                                {getWeightUnit()}
                            </td>
                        </tr>
                        <tr>
                            <td>{translate("water_ratio")}:</td>
                            <td>
                                {formatPercent(measures.water_ratio / 100, 1)}
                            </td>
                        </tr>
                        <tr>
                            <td>{translate("water_mass")}:</td>
                            <td>
                                {formatNumber(
                                    fromKg(
                                        measures.weight *
                                            (measures.water_ratio / 100),
                                    ),
                                    1,
                                )}{" "}
                                {getWeightUnit()}
                            </td>
                        </tr>
                    </tbody>
                </table>
                <div>
                    <hr />
                    <Button
                        id="import-button"
                        className="full-width-button"
                        variant="contained"
                        color="error"
                        onClick={deleteEntry}
                    >
                        {translate("delete_entry")}
                    </Button>
                </div>
            </DialogContent>
        </Dialog>
    );
}
