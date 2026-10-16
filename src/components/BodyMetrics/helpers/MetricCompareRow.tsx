import { BodyMetricListItem } from "@/utils/backend/models";
import { ReactNode } from "react";

type Props = {
    label: string;
    measures: BodyMetricListItem[];
    render: (entry: BodyMetricListItem) => ReactNode;
};

export function MetricCompareRow({ label, measures, render }: Props) {
    return (
        <tr>
            <td>{label}:</td>
            {measures.map((entry, idx) => (
                <td key={"entry-" + idx}>{render(entry)}</td>
            ))}
        </tr>
    );
}
