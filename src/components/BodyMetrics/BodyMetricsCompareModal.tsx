import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { BodyMetricListItem } from "@/utils/backend/models";
import { useContext } from "react";
import { Dialog, DialogContent, DialogTitle, IconButton } from "@mui/material";
import CloseIcon from "@mui/icons-material/Close";
import { MetricCompareRow } from "./helpers/MetricCompareRow";

type Props = {
    measures: BodyMetricListItem[];
    onClose: () => void;
};

export function BodyMetricsCompareModal({ measures, onClose }: Props) {
    const {
        translate,
        fromKg,
        getWeightUnit,
        formatDate,
        formatNumber,
        formatPercent,
    } = useContext(I18nSettingsContext);

    return (
        <Dialog open={true} onClose={onClose}>
            <DialogTitle>
                {translate("metrics_comparaison")}
                <IconButton onClick={onClose} className="modal-close-button">
                    <CloseIcon />
                </IconButton>
            </DialogTitle>

            <DialogContent dividers>
                <table id="workout-details-table">
                    <colgroup>
                        <col className="col-200" />
                        <col className="col-150" />
                        <col className="col-150" />
                        <col />
                    </colgroup>
                    <thead>
                        <tr>
                            <td className="divider-bottom-white"></td>
                            {measures.map((entry, idx) => (
                                <td
                                    key={"entry-" + idx}
                                    className="divider-bottom-white"
                                >
                                    <b>{formatDate(entry.date)}</b>
                                </td>
                            ))}
                        </tr>
                    </thead>
                    <tbody>
                        <MetricCompareRow
                            label={translate("weight")}
                            measures={measures}
                            render={(entry) => (
                                <>
                                    {formatNumber(fromKg(entry.weight), 1)}{" "}
                                    {getWeightUnit()}
                                </>
                            )}
                        />
                        <MetricCompareRow
                            label={translate("fat_ratio")}
                            measures={measures}
                            render={(entry) =>
                                formatPercent(entry.fat_ratio / 100, 1)
                            }
                        />
                        <MetricCompareRow
                            label={translate("fat_mass")}
                            measures={measures}
                            render={(entry) => (
                                <>
                                    {formatNumber(
                                        fromKg(
                                            entry.weight *
                                                (entry.fat_ratio / 100),
                                        ),
                                        1,
                                    )}{" "}
                                    {getWeightUnit()}
                                </>
                            )}
                        />
                        <MetricCompareRow
                            label={translate("lean_mass")}
                            measures={measures}
                            render={(entry) => (
                                <>
                                    {formatNumber(fromKg(entry.lean_mass), 1)}{" "}
                                    {getWeightUnit()}
                                </>
                            )}
                        />
                        <MetricCompareRow
                            label={translate("water_ratio")}
                            measures={measures}
                            render={(entry) =>
                                formatPercent(entry.water_ratio / 100, 1)
                            }
                        />
                        <MetricCompareRow
                            label={translate("water_mass")}
                            measures={measures}
                            render={(entry) => (
                                <>
                                    {formatNumber(
                                        fromKg(
                                            entry.weight *
                                                (entry.water_ratio / 100),
                                        ),
                                        1,
                                    )}{" "}
                                    {getWeightUnit()}
                                </>
                            )}
                        />
                    </tbody>
                </table>
            </DialogContent>
        </Dialog>
    );
}
