import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { SessionSet } from "@/utils/backend/models";
import { SessionUtils } from "@/utils/SessionUtils";
import { useContext } from "react";
import {
    CartesianGrid,
    Legend,
    Line,
    LineChart,
    ResponsiveContainer,
    Tooltip,
    TooltipContentProps,
    XAxis,
    YAxis,
} from "recharts";

type Props = {
    unilateral: boolean;
    series: Record<string, SessionSet[]>;
};

function ExerciseVolumeTooltip({
    active,
    payload,
    label,
}: TooltipContentProps) {
    const { translate, formatDate, formatNumber, fromKg } =
        useContext(I18nSettingsContext);

    if (!active || !payload || payload.length === 0 || label == null) {
        return null;
    }
    const data = payload[0].payload as {
        date: number;
        weight: number;
        reps: number;
        e1rm: number;
    };
    return (
        <div className="chart-tooltip">
            <div>
                <b>{formatDate(data.date)}</b>
            </div>
            <div>
                {translate("rm")}: {formatNumber(fromKg(data.e1rm), 1)}
            </div>
            <div>
                {translate("weight")}: {formatNumber(fromKg(data.weight), 1)}
            </div>
            <div>
                {translate("repetitions")}: {formatNumber(data.reps, 0)}
            </div>
        </div>
    );
}

export function ExerciseVolumeChart({ series, unilateral }: Props) {
    const { translate } = useContext(I18nSettingsContext);
    const chartData: {
        date: number;
        weight: number;
        reps: number;
        e1rm: number;
    }[] = [];

    Object.keys(series).forEach((k) => {
        const date = new Date(parseInt(k.split("\n")[1]));

        let count = 0;
        let weight = 0;
        series[k].forEach((s) => {
            count += s.reps;
            weight = Math.max(weight, s.weight);
        });
        chartData.push({
            date: date.getTime(),
            weight,
            reps: count,
            e1rm: SessionUtils.maxE1rm(series[k], unilateral),
        });
    });
    chartData.sort((a, b) => a.date - b.date);
    const dates = chartData.map(({ date }) => date);
    const minDate = Math.min(...dates);
    const maxDate = Math.max(...dates);
    const maxWeight = Math.max(...chartData.map(({ e1rm }) => e1rm));
    const minReps = Math.min(...chartData.map(({ reps }) => reps));
    const maxReps = Math.max(...chartData.map(({ reps }) => reps));

    return (
        <div className="chart-container">
            <ResponsiveContainer width="100%" height="100%">
                <LineChart
                    data={chartData}
                    margin={{ top: 5, right: 5, left: 5, bottom: 5 }}
                >
                    <CartesianGrid stroke="#80808000" strokeDasharray="5 5" />
                    <XAxis
                        dataKey="date"
                        type="number"
                        domain={[minDate, maxDate]}
                        stroke="#fff"
                        tick={false}
                        height={0}
                    />
                    <YAxis
                        yAxisId="left"
                        stroke="#fff"
                        width={0}
                        domain={[0, maxWeight]}
                        tick={false}
                    />{" "}
                    <YAxis
                        yAxisId="right"
                        stroke="#fff"
                        domain={[minReps, maxReps]}
                        width={0}
                        tick={false}
                    />{" "}
                    <Line
                        yAxisId="left"
                        type="monotone"
                        name={translate("rm")}
                        dataKey="e1rm"
                        stroke="#f00"
                        dot={{ fill: "#f00" }}
                        activeDot={{ stroke: "#f0f0f000" }}
                        isAnimationActive={false}
                    />
                    <Line
                        yAxisId="left"
                        type="monotone"
                        name={translate("weight")}
                        dataKey="weight"
                        stroke="#0f0"
                        dot={{ fill: "#0f0" }}
                        activeDot={{ stroke: "#f0f0f000" }}
                        isAnimationActive={false}
                    />
                    <Line
                        yAxisId="right"
                        type="monotone"
                        name={translate("repetitions")}
                        dataKey="reps"
                        stroke="#0ff"
                        dot={{ fill: "#0ff" }}
                        activeDot={{ stroke: "#f0f0f000" }}
                        isAnimationActive={false}
                    />
                    <Tooltip
                        content={(props) => (
                            <ExerciseVolumeTooltip {...props} />
                        )}
                    />
                    <Legend />
                </LineChart>
            </ResponsiveContainer>
        </div>
    );
}
