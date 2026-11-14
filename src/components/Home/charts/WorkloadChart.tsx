import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { LoadZone, SessionUtils, WorkoutLoad } from "@/utils/SessionUtils";
import { useContext } from "react";
import {
    Area,
    CartesianGrid,
    ComposedChart,
    Line,
    ResponsiveContainer,
    Tooltip,
    TooltipContentProps,
    useXAxisScale,
    useYAxisScale,
    XAxis,
    YAxis,
} from "recharts";

const Y_AXIS = "left";

const ZONE_COLORS: Record<LoadZone, string> = {
    above: "#ef5350",
    optimal: "green",
    below: "#42a5f5",
};

/** Acute load line, colored by its position relative to the optimal range. */
function ZoneLine({ workload }: { workload: WorkoutLoad[] }) {
    const xScale = useXAxisScale();
    const yScale = useYAxisScale(Y_AXIS);
    if (!xScale || !yScale) {
        return null;
    }

    return (
        <g>
            {SessionUtils.loadSegments(workload).map((s, i) => (
                <line
                    key={i}
                    x1={xScale(s.x0)}
                    y1={yScale(s.y0)}
                    x2={xScale(s.x1)}
                    y2={yScale(s.y1)}
                    stroke={ZONE_COLORS[s.zone]}
                    strokeWidth={1.5}
                    strokeLinecap="round"
                />
            ))}
            {workload.map((p) => (
                <circle
                    key={p.date}
                    cx={xScale(p.date)}
                    cy={yScale(p.current)}
                    r={3}
                    fill={
                        ZONE_COLORS[
                            SessionUtils.loadZone(
                                p.current,
                                p.lower,
                                p.lower + p.upper,
                            )
                        ]
                    }
                />
            ))}
        </g>
    );
}

type Props = {
    workload: WorkoutLoad[];
    minDate: number;
};

function WorkloadTooltip({ active, payload, label }: TooltipContentProps) {
    const { translate, formatDate, formatNumber } =
        useContext(I18nSettingsContext);

    if (!active || !payload || payload.length === 0 || label == null) {
        return null;
    }
    const data = payload[0].payload as WorkoutLoad;
    return (
        <div className="chart-tooltip">
            <div>
                <b>{formatDate(data.date / 1000)}</b>
            </div>
            <div>
                {translate("upper_threshold")}:{" "}
                {formatNumber(data.lower + data.upper, 0)}
            </div>
            <div>
                {translate("workload")}: {formatNumber(data.current, 0)}
            </div>
            <div>
                {translate("lower_threshold")}: {formatNumber(data.lower, 0)}
            </div>
            {data.reference > 0 && (
                <div>
                    {translate("load_ratio")}:{" "}
                    {/* Truncated to one decimal, as Garmin shows it */}
                    {formatNumber(
                        Math.floor((data.current / data.reference) * 10) / 10,
                        1,
                    )}
                </div>
            )}
        </div>
    );
}

export function WorkloadChart({ workload, minDate }: Props) {
    const { translate } = useContext(I18nSettingsContext);

    // Status of today's load
    const today = workload[workload.length - 1];
    const zone = SessionUtils.loadZone(
        today.current,
        today.lower,
        today.lower + today.upper,
    );

    // Y axis spans just the load line and the optimal range
    const yMin = Math.min(...workload.map((p) => Math.min(p.current, p.lower)));
    const yMax = Math.max(
        ...workload.map((p) => Math.max(p.current, p.lower + p.upper)),
    );
    const yDomain = yMax > yMin ? [yMin, yMax] : [yMin, yMin + 1];

    return (
        <div className="chart-container workload-chart">
            <div
                className="workload-status"
                style={{ color: ZONE_COLORS[zone] }}
            >
                {translate("load_status_" + zone)}
            </div>
            <ResponsiveContainer
                className="chart-responsive"
                width="100%"
                height="100%"
            >
                <ComposedChart data={workload}>
                    <CartesianGrid stroke="#80808000" strokeDasharray="5 5" />
                    <XAxis
                        dataKey="date"
                        type="number"
                        domain={[minDate, workload[workload.length - 1].date]}
                        stroke="#fff"
                        tick={false}
                        height={0}
                    />
                    <YAxis
                        yAxisId={Y_AXIS}
                        stroke="#fff"
                        width={0}
                        domain={yDomain}
                        // Otherwise the stacked band would stretch the axis down to 0
                        allowDataOverflow
                        tick={false}
                    />
                    {/* Straight band edges, so the line changes color right where it crosses them */}
                    <Area
                        yAxisId={Y_AXIS}
                        dataKey="lower"
                        stackId="1"
                        stroke="none"
                        type="linear"
                        legendType="none"
                        fill="transparent"
                        dot={false}
                        isAnimationActive={false}
                        activeDot={false}
                    />
                    <Area
                        yAxisId={Y_AXIS}
                        dataKey="upper"
                        stackId="1"
                        stroke="none"
                        type="linear"
                        fill="lightgreen"
                        legendType="none"
                        fillOpacity={0.1}
                        dot={false}
                        isAnimationActive={false}
                        activeDot={false}
                    />
                    <Line
                        yAxisId={Y_AXIS}
                        type="monotone"
                        name={translate("reference")}
                        legendType="line"
                        dataKey="reference"
                        stroke="#ffffff40"
                        dot={false}
                        isAnimationActive={false}
                        activeDot={false}
                    />
                    <ZoneLine workload={workload} />
                    <Tooltip
                        isAnimationActive={false}
                        content={(props) => <WorkloadTooltip {...props} />}
                    />
                </ComposedChart>
            </ResponsiveContainer>
        </div>
    );
}
