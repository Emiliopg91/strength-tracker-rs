import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { BodyMetricListItem } from "@/utils/backend/models";
import { useContext } from "react";

type Props = {
    measure: BodyMetricListItem;
    selectedForCompare: boolean;
    onSelect: (measure: BodyMetricListItem) => void;
    onToggleCompare: (measure: BodyMetricListItem) => void;
};

export function BodyMetricRow({
    measure,
    selectedForCompare,
    onSelect,
    onToggleCompare,
}: Props) {
    const { fromKg, getWeightUnit, formatDate, formatNumber, formatPercent } =
        useContext(I18nSettingsContext);

    return (
        <tr
            onClick={() => onSelect(measure)}
            onContextMenu={(e) => {
                e.preventDefault();
                onToggleCompare(measure);
            }}
            className={`clickable-row ${selectedForCompare ? "row-compare-selected" : ""}`}
        >
            <td>{formatDate(measure.date)}</td>
            <td>
                {formatNumber(fromKg(measure.weight), 1)} {getWeightUnit()}
            </td>
            <td>{formatPercent(measure.fat_ratio / 100, 1)}</td>
            <td>
                {formatNumber(
                    fromKg((measure.fat_ratio / 100) * measure.weight),
                    1,
                )}{" "}
                {getWeightUnit()}
            </td>
            <td>
                {formatNumber(fromKg(measure.lean_mass), 1)} {getWeightUnit()}
            </td>
            <td>{formatPercent(measure.water_ratio / 100, 1)}</td>
            <td>
                {formatNumber(
                    fromKg((measure.water_ratio / 100) * measure.weight),
                    1,
                )}{" "}
                {getWeightUnit()}
            </td>
        </tr>
    );
}
