import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { SessionSet } from "@/utils/backend/models";
import { SessionUtils } from "@/utils/SessionUtils";
import { useContext, useEffect, useState } from "react";
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
        volume: number;
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
                {translate("volume")}: {formatNumber(fromKg(data.volume), 1)}
            </div>
            <div>
                {translate("repetitions")}: {formatNumber(data.reps, 0)}
            </div>
        </div>
    );
}

export function ExerciseVolumeChart({ series }: Props) {
    const { translate } = useContext(I18nSettingsContext);
    const [chartData, setChartData] = useState<
        { date: number; volume: number; reps: number; e1rm: number }[]
    >([]);
    const [maxVol, setMaxVol] = useState(0);
    const [minDate, setMinDate] = useState(99999);
    const [maxDate, setMaxDate] = useState(0);

    useEffect(() => {
        const data: {
            date: number;
            volume: number;
            reps: number;
            e1rm: number;
        }[] = [];

        Object.keys(series).forEach((k) => {
            const date = new Date(parseInt(k.split("\n")[1]));

            let count = 0;
            let weight = 0;
            series[k].forEach((s) => {
                count += s.reps;
                weight += s.reps * s.weight;
            });
            data.push({
                date: date.getTime(),
                volume: weight,
                reps: count,
                e1rm: SessionUtils.maxE1rm(series[k]),
            });
        });
        data.sort((a, b) => a.date - b.date);
        setChartData(data);
        const dates = [...data].map(({ date }) => {
            return date;
        });
        setMinDate(Math.min(...dates));
        setMaxDate(Math.max(...dates));
        const volumes = data.map(({ volume }) => {
            return volume;
        });
        setMaxVol(Math.max(...volumes));
    }, [series]);

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
                        domain={[0, maxVol]}
                        tick={false}
                    />{" "}
                    <YAxis
                        yAxisId="right"
                        stroke="#fff"
                        width={0}
                        tick={false}
                    />{" "}
                    <Line
                        yAxisId="right"
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
                        name={translate("volume")}
                        dataKey="volume"
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
