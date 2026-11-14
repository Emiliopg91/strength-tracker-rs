import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { useContext, useState } from "react";
import {
    ResponsiveContainer,
    Scatter,
    ScatterChart,
    Tooltip,
    XAxis,
    YAxis,
    type ScatterShapeProps,
} from "recharts";
import "@/styles/Home/Home.css";

interface HeatMapPoint {
    month: number;
    day: number;
    load: number;
    records: number;
    row: number;
}
const monthToRow = (
    month: number,
    day: number,
    todayMonth: number,
    todayDay: number,
): number => {
    if (month === todayMonth) {
        return day > todayDay ? 0 : 12;
    }
    return ((month - todayMonth - 1 + 12) % 12) + 1;
};

const rowToMonth = (row: number, todayMonth: number): number => {
    if (row === 0 || row === 12) {
        return todayMonth;
    }
    return ((row - 1 + todayMonth) % 12) + 1;
};

const yearForMonth = (
    month: number,
    day: number,
    todayMonth: number,
    todayDay: number,
    todayYear: number,
): number => {
    if (month === todayMonth) {
        return day > todayDay ? todayYear - 1 : todayYear;
    }
    return month < todayMonth ? todayYear : todayYear - 1;
};

const isValidCalendarDate = (
    year: number,
    month: number,
    day: number,
): boolean => {
    const date = new Date(year, month - 1, day);
    return (
        date.getFullYear() === year &&
        date.getMonth() === month - 1 &&
        date.getDate() === day
    );
};

const HEATMAP_ZERO_COLOR = "#000000";
const HEATMAP_MAX_COLOR = "#00ff00";
const HEATMAP_SCALE_MAX = 100;
const HEATMAP_Y_AXIS_WIDTH = 65;
const HEATMAP_X_AXIS_HEIGHT = 10;
const HEATMAP_DAY_TICKS = [1, 10, 20, 30];
const HEATMAP_ROW_TICKS = [0, 2, 4, 6, 8, 10, 12];

const hexToRgb = (hex: string): [number, number, number] => [
    parseInt(hex.slice(1, 3), 16),
    parseInt(hex.slice(3, 5), 16),
    parseInt(hex.slice(5, 7), 16),
];

const getHeatColor = (load: number): string => {
    const [r, g, b] =
        load == 0 ? hexToRgb(HEATMAP_ZERO_COLOR) : hexToRgb(HEATMAP_MAX_COLOR);
    const alpha =
        load == 0 ? 0.1 : 0.1 + 0.9 * Math.min(1, load / HEATMAP_SCALE_MAX);
    return `rgba(${r}, ${g}, ${b}, ${alpha})`;
};

interface HeatmapProps {
    data: [number, number][][] | undefined;
}

export function Heatmap({ data }: HeatmapProps) {
    const { translate, formatDate, formatNumber } =
        useContext(I18nSettingsContext);

    const [size, setSize] = useState({ width: 0, height: 0 });
    // Measures on attach and on window resize; the returned cleanup runs on detach
    const setContainerRef = (el: HTMLDivElement) => {
        const measure = () => {
            const { width, height } = el.getBoundingClientRect();
            // Keep the previous object when unchanged, so re-attaching the ref can't loop
            setSize((previous) =>
                previous.width === width && previous.height === height
                    ? previous
                    : { width, height },
            );
        };
        measure();
        window.addEventListener("resize", measure);
        return () => {
            window.removeEventListener("resize", measure);
        };
    };

    if (!data) {
        return null;
    }

    const today = new Date();
    const todayMonth = today.getMonth() + 1;
    const todayDay = today.getDate();
    const todayYear = today.getFullYear();

    // Row 0 holds last year's days of the current month that come after today;
    // when today is the last day of that month the row has no cells at all.
    const isTopRowEmpty =
        todayDay >= new Date(todayYear - 1, todayMonth, 0).getDate();

    const isValidPoint = (point: HeatMapPoint) =>
        isValidCalendarDate(
            yearForMonth(
                point.month,
                point.day,
                todayMonth,
                todayDay,
                todayYear,
            ),
            point.month,
            point.day,
        );

    const points: HeatMapPoint[] = data.flatMap((monthLoads, m) => {
        const month = m + 1;
        return monthLoads.map((data, d) => {
            const day = d + 1;
            return {
                month,
                day,
                load: data[0],
                records: data[1],
                row: monthToRow(month, day, todayMonth, todayDay),
            };
        });
    });
    const rawCellWidth = (size.width - HEATMAP_Y_AXIS_WIDTH) / 31;
    const rawCellHeight = (size.height - HEATMAP_X_AXIS_HEIGHT) / 13;
    const cellSize = Math.max(0, Math.min(rawCellWidth, rawCellHeight));
    // Shrink the plot area itself to exactly cellSize * 31/13, so consecutive
    // points sit cellSize apart (true square tiling) - the leftover space
    // becomes a margin around the whole grid instead of gaps between cells.
    const extraWidth = Math.max(
        0,
        size.width - HEATMAP_Y_AXIS_WIDTH - cellSize * 31,
    );
    const extraHeight = Math.max(
        0,
        size.height - HEATMAP_X_AXIS_HEIGHT - cellSize * 13,
    );

    const renderCell = (props: ScatterShapeProps) => {
        const { cx, cy, payload } = props;
        if (cx === undefined || cy === undefined) {
            return null;
        }
        const point = payload as HeatMapPoint;
        if (!isValidPoint(point)) {
            return null;
        }
        const width = Math.max(0, cellSize - 1);
        const height = width;
        const isRecord = point.records > 0;
        return (
            <rect
                x={cx - width / 2}
                y={cy - height / 2}
                width={width}
                height={height}
                rx={1}
                fill={getHeatColor(point.load)}
                stroke={isRecord ? "#FFFF00" : "none"}
                strokeWidth={isRecord ? 2 : 0}
            />
        );
    };

    return (
        <div className="chart-container" ref={setContainerRef}>
            <ResponsiveContainer
                className="chart-responsive"
                width="100%"
                height="100%"
            >
                <ScatterChart
                    margin={{
                        top: extraHeight / 2,
                        bottom: extraHeight / 2,
                        left: extraWidth / 2,
                        right: extraWidth / 2,
                    }}
                >
                    <XAxis
                        dataKey="day"
                        type="number"
                        domain={[0.5, 31.5]}
                        stroke="#fff"
                        ticks={HEATMAP_DAY_TICKS}
                        tick={{ fill: "#fff", fontSize: 10 }}
                        axisLine={false}
                        tickLine={false}
                        height={HEATMAP_X_AXIS_HEIGHT}
                    />
                    <YAxis
                        dataKey="row"
                        type="number"
                        domain={[-0.5, 12.5]}
                        reversed
                        stroke="#fff"
                        ticks={HEATMAP_ROW_TICKS}
                        tickFormatter={(value: number) =>
                            value === 0 && isTopRowEmpty
                                ? ""
                                : translate(
                                      "month_" + rowToMonth(value, todayMonth),
                                  )
                        }
                        tick={{ fill: "#fff", fontSize: 10 }}
                        axisLine={false}
                        tickLine={false}
                        width={HEATMAP_Y_AXIS_WIDTH}
                    />
                    <Tooltip
                        cursor={false}
                        isAnimationActive={false}
                        content={(props) => {
                            const point = props.payload?.[0]?.payload as
                                HeatMapPoint | undefined;
                            if (
                                !props.active ||
                                !point ||
                                !isValidPoint(point)
                            ) {
                                return null;
                            }
                            const year = yearForMonth(
                                point.month,
                                point.day,
                                todayMonth,
                                todayDay,
                                todayYear,
                            );
                            const date = new Date(
                                year,
                                point.month - 1,
                                point.day,
                            );
                            return (
                                <div className="heatmap-tooltip">
                                    <b>{formatDate(date.getTime() / 1000)}</b>
                                    <br />
                                    <span>
                                        {translate("workout_load") +
                                            ": " +
                                            formatNumber(point.load, 0)}
                                    </span>
                                    <br />
                                    <span>
                                        {translate("records") +
                                            ": " +
                                            formatNumber(point.records, 0)}
                                    </span>
                                </div>
                            );
                        }}
                    />
                    <Scatter
                        data={points}
                        shape={renderCell}
                        isAnimationActive={false}
                    />
                </ScatterChart>
            </ResponsiveContainer>
        </div>
    );
}
