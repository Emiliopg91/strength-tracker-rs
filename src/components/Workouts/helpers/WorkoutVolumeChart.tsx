import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { WorkoutSession } from "@/utils/backend/models";
import { useContext } from "react";
import {
    CartesianGrid,
    Legend,
    Line,
    LineChart,
    ReferenceLine,
    ResponsiveContainer,
    Tooltip,
    TooltipContentProps,
    XAxis,
    YAxis,
} from "recharts";

type Props = {
    sessions: WorkoutSession[];
};

function WorkoutVolumeTooltip({ active, payload, label }: TooltipContentProps) {
    const { translate, formatDate, formatNumber } =
        useContext(I18nSettingsContext);

    if (!active || !payload || payload.length === 0 || label == null) {
        return null;
    }
    const data = payload[0].payload as { date: number; volume: number };
    return (
        <div className="chart-tooltip">
            <div>
                <b>{formatDate(data.date / 1000)}</b>
            </div>
            <div>
                {translate("volume")}: {formatNumber(data.volume, 0)}
            </div>
        </div>
    );
}

export function WorkoutVolumeChart({ sessions }: Props) {
    const { translate } = useContext(I18nSettingsContext);
    const chartData = sessions.toReversed().map((ws) => {
        const day = new Date(ws.date * 1000);
        const date = new Date(day.getFullYear(), day.getMonth(), day.getDate());
        return {
            date: date.getTime(),
            volume: ws.volume,
        };
    });
    const dates = chartData.map(({ date }) => date);
    const minDate = Math.min(...dates);
    const maxDate = Math.max(...dates);
    const volumes = chartData.map(({ volume }) => volume);
    const minVol = Math.min(...volumes);
    const maxVol = Math.max(...volumes);

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
                        stroke="#fff"
                        type="number"
                        domain={[minDate, maxDate]}
                        tick={false}
                        height={0}
                    />
                    <YAxis
                        stroke="#fff"
                        width={0}
                        domain={[minVol * 0.9, maxVol * 1.1]}
                        tick={false}
                    />
                    <Line
                        name={translate("volume")}
                        type="monotone"
                        dataKey="volume"
                        stroke="#0f0"
                        dot={{ fill: "#0f0" }}
                        activeDot={{ stroke: "#00ff0000" }}
                        isAnimationActive={false}
                    />
                    <ReferenceLine
                        y={(minVol + maxVol) / 2}
                        stroke="#808080"
                        strokeDasharray="10 5"
                    />
                    <Tooltip
                        isAnimationActive={false}
                        content={(props) => <WorkoutVolumeTooltip {...props} />}
                    />
                    <Legend />
                </LineChart>
            </ResponsiveContainer>
        </div>
    );
}
