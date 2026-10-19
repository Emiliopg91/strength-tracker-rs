import { JSX, useContext } from "react";
import PageContent from "./PageContent";
import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { BodyMetricListItem } from "@/utils/backend/models";

type Props = {
    bodyMetrics: BodyMetricListItem[];
};

export default function BodySection({ bodyMetrics }: Props): JSX.Element {
    const {
        translate,
        formatDate,
        formatNumber,
        fromKg,
        formatPercent,
        getWeightUnit,
    } = useContext(I18nSettingsContext);

    return (
        <>
            {bodyMetrics.length > 0 && (
                <PageContent>
                    <h2>{translate("body_composition")}</h2>
                    <div className="section-container">
                        <div className="report-table">
                            <table>
                                <thead>
                                    <tr>
                                        <th className="text-center">
                                            {translate("date")}
                                        </th>
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
                                    {bodyMetrics.map((measure) => (
                                        <tr key={measure.date}>
                                            <td>{formatDate(measure.date)}</td>
                                            <td>
                                                {formatNumber(
                                                    fromKg(measure.weight),
                                                    1,
                                                )}{" "}
                                                {getWeightUnit()}
                                            </td>
                                            <td>
                                                {formatPercent(
                                                    measure.fat_ratio / 100,
                                                    1,
                                                )}
                                            </td>
                                            <td>
                                                {formatNumber(
                                                    fromKg(
                                                        (measure.fat_ratio /
                                                            100) *
                                                            measure.weight,
                                                    ),
                                                    1,
                                                )}{" "}
                                                {getWeightUnit()}
                                            </td>
                                            <td>
                                                {formatNumber(
                                                    fromKg(measure.lean_mass),
                                                    1,
                                                )}{" "}
                                                {getWeightUnit()}
                                            </td>
                                            <td>
                                                {formatPercent(
                                                    measure.water_ratio / 100,
                                                    1,
                                                )}
                                            </td>
                                            <td>
                                                {formatNumber(
                                                    fromKg(
                                                        (measure.water_ratio /
                                                            100) *
                                                            measure.weight,
                                                    ),
                                                    1,
                                                )}{" "}
                                                {getWeightUnit()}
                                            </td>
                                        </tr>
                                    ))}
                                </tbody>
                            </table>
                        </div>
                    </div>
                </PageContent>
            )}
        </>
    );
}
