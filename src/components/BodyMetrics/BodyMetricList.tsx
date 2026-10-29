import { BackendClient } from "@/utils/backend/client";
import { useContext, useState } from "react";
import { useBackendList } from "@/hooks/useBackendList";
import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { BodyMetricListItem } from "@/utils/backend/models";
import { BodyMetricsDetailsModal } from "./BodyMetricsDetailsModal";
import { Button } from "@mui/material";
import { BodyMetricsAddModal } from "./BodyMetricsAddModal";
import { BodyMetricsChart } from "./helpers/BodyMetricsChart";
import { BodyMetricRow } from "./helpers/BodyMetricRow";
import { BodyMetricsCompareModal } from "./BodyMetricsCompareModal";
import { AddBodyMetricMenu } from "./helpers/AddBodyMetricMenu";
import "@/styles/BodyMetrics/BodyMetrics.css";

export function BodyMetricList() {
    const { translate } = useContext(I18nSettingsContext);

    const [bodyMetrics, , refreshList] = useBackendList(
        BackendClient.getBodyMeasures,
        [],
    );
    const [addingNew, setAddingNew] = useState(false);
    const [measureDetails, setMeasureDetails] = useState<
        BodyMetricListItem | undefined
    >(undefined);

    const [toCompare, setToCompare] = useState<number[]>([]);
    const [comparingSessions, setComparingSessions] = useState<
        BodyMetricListItem[]
    >([]);

    const goToCompare = () => {
        const sorted = toCompare.toSorted((a, b) => a - b);
        const sessions = sorted.map((date) =>
            bodyMetrics.find((entry) => entry.date == date)!,
        );
        setComparingSessions(sessions);
    };

    const toggleSelect = (metric: BodyMetricListItem) => {
        if (toCompare.includes(metric.date)) {
            setToCompare([...toCompare].filter((date) => date != metric.date));
        } else {
            if (toCompare.length < 3) {
                setToCompare([...toCompare, metric.date]);
            }
        }
    };

    const openModal = (details: BodyMetricListItem) => {
        setMeasureDetails(details);
    };

    return (
        <>
            <div id="list-layer">
                {bodyMetrics.length > 1 && (
                    <BodyMetricsChart metrics={bodyMetrics} />
                )}

                <table>
                    <thead>
                        <tr>
                            <th className="text-center">{translate("date")}</th>
                            <th className="text-center">
                                {translate("weight")}
                            </th>
                            <th className="text-center">
                                {translate("fat_ratio")}
                            </th>
                            <th className="text-center">
                                {translate("fat_mass")}
                            </th>
                            <th className="text-center">
                                {translate("lean_mass")}
                            </th>
                            <th className="text-center">
                                {translate("water_ratio")}
                            </th>
                            <th className="text-center">
                                {translate("water_mass")}
                            </th>
                        </tr>
                    </thead>
                    <tbody>
                        {bodyMetrics.map((measure, idx) => (
                            <BodyMetricRow
                                key={idx}
                                measure={measure}
                                selectedForCompare={toCompare.includes(
                                    measure.date,
                                )}
                                onSelect={openModal}
                                onToggleCompare={toggleSelect}
                            />
                        ))}
                    </tbody>
                </table>
            </div>

            <div>
                {measureDetails && (
                    <BodyMetricsDetailsModal
                        measures={measureDetails}
                        onClose={() => {
                            setMeasureDetails(undefined);
                        }}
                        onDelete={refreshList}
                    />
                )}
                {addingNew && (
                    <BodyMetricsAddModal
                        onClose={() => {
                            setAddingNew(false);
                            refreshList();
                        }}
                        latest={
                            bodyMetrics.length > 0 ? bodyMetrics[0] : undefined
                        }
                    />
                )}
                {comparingSessions.length >= 2 && (
                    <BodyMetricsCompareModal
                        measures={comparingSessions}
                        onClose={() => setComparingSessions([])}
                    />
                )}
            </div>
            <div className="list-action-bar list-action-bar-row">
                {toCompare.length >= 2 && (
                    <Button
                        variant="contained"
                        className="full-width-button mr-5"
                        onClick={goToCompare}
                    >
                        {translate("compare")}
                    </Button>
                )}
                <AddBodyMetricMenu
                    onAddManually={() => {
                        setAddingNew(true);
                    }}
                    onImported={refreshList}
                />
            </div>
        </>
    );
}
