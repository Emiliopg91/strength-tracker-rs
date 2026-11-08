import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { ExerciseDetails, ExerciseSession } from "@/utils/backend/models";
import { ExerciseChartPoint, SessionUtils } from "@/utils/SessionUtils";
import { useContext, useState } from "react";
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
    exercise: ExerciseDetails;
    series: Record<string, ExerciseSession>;
};

type LineKey = "weight" | "reps" | "relativeWeight";

const INACTIVE_COLOR = "#808080";

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
    const data = payload[0].payload as ExerciseChartPoint;
    return (
        <div className="chart-tooltip">
            <div>
                <b>{formatDate(data.date)}</b>
            </div>
            <div>
                {translate("weight")}: {formatNumber(fromKg(data.weight), 1)}
            </div>
            <div>
                {translate("repetitions")}: {formatNumber(data.reps, 0)}
            </div>
            {data.relativeWeight !== null && (
                <div>
                    {translate("relative_weight")}:{" "}
                    {formatNumber(data.relativeWeight, 2)}
                </div>
            )}
        </div>
    );
}

export function ExerciseVolumeChart({ series, exercise }: Props) {
    const { translate } = useContext(I18nSettingsContext);
    const [hidden, setHidden] = useState<Set<LineKey>>(new Set());

    const chartData = SessionUtils.exerciseChartData(
        series,
        exercise.chart_session,
        exercise.unilateral,
    );
    const anyRelative =
        chartData.find((e) => e.relativeWeight !== null) !== undefined;

    const domain = (values: number[]) => [
        Math.min(...values),
        Math.max(...values),
    ];
    const dateDomain = domain(chartData.map(({ date }) => date));
    const weightDomain = domain(chartData.map(({ weight }) => weight));
    const repsDomain = domain(chartData.map(({ reps }) => reps));
    const rmDomain = domain(
        chartData
            .map(({ relativeWeight }) => relativeWeight)
            .filter((v) => v !== null),
    );

    const toggle = (key: LineKey) =>
        setHidden((prev) => {
            const next = new Set(prev);
            if (!next.delete(key)) {
                next.add(key);
            }
            return next;
        });

    const lines: {
        key: LineKey;
        name: string;
        color: string;
        dashed?: boolean;
    }[] = [
        { key: "weight", name: translate("weight"), color: "#0f0" },
        { key: "reps", name: translate("repetitions"), color: "#0ff" },
    ];
    if (anyRelative) {
        lines.push({
            key: "relativeWeight",
            name: translate("relative_weight"),
            color: "#f80",
            dashed: true,
        });
    }

    return (
        <>
            {chartData.length > 1 && (
                <>
                    <div className="chart-container">
                        <ResponsiveContainer width="100%" height="100%">
                            <LineChart
                                data={chartData}
                                margin={{
                                    top: 5,
                                    right: 5,
                                    left: 5,
                                    bottom: 5,
                                }}
                            >
                                <CartesianGrid
                                    stroke="#80808000"
                                    strokeDasharray="5 5"
                                />
                                <XAxis
                                    dataKey="date"
                                    type="number"
                                    domain={dateDomain}
                                    stroke="#fff"
                                    tick={false}
                                    height={0}
                                />
                                <YAxis
                                    yAxisId="weight"
                                    stroke="#fff"
                                    width={0}
                                    domain={weightDomain}
                                    tick={false}
                                />
                                <YAxis
                                    yAxisId="reps"
                                    stroke="#fff"
                                    width={0}
                                    domain={repsDomain}
                                    tick={false}
                                />
                                {anyRelative && (
                                    <YAxis
                                        yAxisId="relativeWeight"
                                        stroke="#fff"
                                        width={0}
                                        domain={rmDomain}
                                        tick={false}
                                    />
                                )}
                                {lines.map(({ key, name, color, dashed }) => (
                                    <Line
                                        key={key}
                                        yAxisId={key}
                                        type="monotone"
                                        name={name}
                                        dataKey={key}
                                        stroke={color}
                                        strokeDasharray={
                                            dashed ? "10 10" : undefined
                                        }
                                        legendType="plainline"
                                        connectNulls
                                        dot={{ fill: color }}
                                        activeDot={{ stroke: "#f0f0f000" }}
                                        hide={hidden.has(key)}
                                        isAnimationActive={false}
                                    />
                                ))}
                                <Tooltip
                                    isAnimationActive={false}
                                    content={(props) => (
                                        <ExerciseVolumeTooltip {...props} />
                                    )}
                                />
                                <Legend
                                    inactiveColor={INACTIVE_COLOR}
                                    wrapperStyle={{ cursor: "pointer" }}
                                    onClick={(entry) =>
                                        toggle(entry.dataKey as LineKey)
                                    }
                                />
                            </LineChart>
                        </ResponsiveContainer>
                    </div>
                    <hr />
                </>
            )}
        </>
    );
}
