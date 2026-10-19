import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { useContext } from "react";
import {
    Area,
    AreaChart,
    CartesianGrid,
    Legend,
    ReferenceLine,
    ResponsiveContainer,
    Tooltip,
    TooltipContentProps,
    XAxis,
    YAxis,
} from "recharts";

type Props = {
    hrBreathData: { idx: number; hr: number; avg: number; color: string }[];
    hrRanges: [number, number, number];
    zonesTimes: number[];
    totalElapsedTime: number;
};

function HeartRateTooltip({
    active,
    payload,
    totalElapsedTime,
    sampleCount,
}: TooltipContentProps & {
    totalElapsedTime: number;
    sampleCount: number;
}) {
    const { translate, formatDuration, formatNumber } =
        useContext(I18nSettingsContext);

    if (!active || !payload || payload.length === 0) {
        return null;
    }
    const data = payload[0].payload as { idx: number; hr: number };
    const elapsed =
        sampleCount > 0 ? (data.idx * totalElapsedTime) / sampleCount : 0;
    return (
        <div className="chart-tooltip">
            <div>
                <b>{formatDuration(elapsed)}</b>
            </div>
            <div>
                {translate("heart_rate")}: {formatNumber(data.hr, 0)}
            </div>
        </div>
    );
}

export function SessionHeartRateChart({
    hrBreathData,
    hrRanges,
    zonesTimes,
    totalElapsedTime,
}: Props) {
    const { translate, formatDuration, formatPercent } =
        useContext(I18nSettingsContext);

    const hrGradientStops = hrBreathData.flatMap((point, i) => {
        const start = (i / hrBreathData.length) * 100;
        const end = ((i + 1) / hrBreathData.length) * 100;
        return [
            <stop
                key={`${i}-start`}
                offset={`${start}%`}
                stopColor={point.color}
            />,
            <stop
                key={`${i}-end`}
                offset={`${end}%`}
                stopColor={point.color}
            />,
        ];
    });

    return (
        <div className="session-hr-chart">
            <ResponsiveContainer width="100%" height="100%">
                <AreaChart
                    data={hrBreathData}
                    margin={{ top: 5, right: 5, left: 5, bottom: 5 }}
                >
                    <defs>
                        <linearGradient
                            id="hrColor"
                            x1="0"
                            y1="0"
                            x2="1"
                            y2="0"
                        >
                            {hrGradientStops}
                        </linearGradient>
                    </defs>
                    <Legend
                        position={"top"}
                        content={() => (
                            <div className="text-center">
                                <div className="session-hr-legend-heading">
                                    {translate("heart_rate")}
                                </div>
                                <div className="session-hr-legend-zones">
                                    {zonesTimes.map(
                                        (time, zone) =>
                                            time > 0 && (
                                                <span
                                                    key={"zone-" + zone}
                                                    className={`session-hr-zone session-hr-zone-${zone + 1}`}
                                                >
                                                    {formatDuration(time) +
                                                        " (" +
                                                        formatPercent(
                                                            time /
                                                                totalElapsedTime,
                                                            0,
                                                        ) +
                                                        ")"}
                                                </span>
                                            ),
                                    )}
                                </div>
                            </div>
                        )}
                    />
                    <CartesianGrid stroke="#80808000" strokeDasharray="5 5" />
                    <XAxis dataKey="idx" tick={false} />
                    <YAxis
                        width="auto"
                        domain={[(4 * hrRanges[0]) / 5, hrRanges[2]]}
                        ticks={hrRanges}
                    />
                    {hrRanges.map((val, idx) => {
                        return (
                            <ReferenceLine
                                key={"range-" + idx}
                                y={val}
                                stroke="white"
                                strokeDasharray="3 3"
                            />
                        );
                    })}
                    <Area
                        dataKey="hr"
                        type="monotone"
                        isAnimationActive={false}
                        stroke="url(#hrColor)"
                        fill="url(#hrColor)"
                        fillOpacity={1}
                        activeDot={false}
                    />
                    <Tooltip
                        content={(props) => (
                            <HeartRateTooltip
                                {...props}
                                totalElapsedTime={totalElapsedTime}
                                sampleCount={hrBreathData.length}
                            />
                        )}
                    />
                </AreaChart>
            </ResponsiveContainer>
        </div>
    );
}
