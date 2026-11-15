import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { BodyMetricListItem } from "@/utils/backend/models";
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

type ChartDataType = {
    date: number;
    fatAvg7: number;
    muscleAvg7: number;
    weightAvg7: number;
}[];

function ChartTooltip({ active, payload, label }: TooltipContentProps) {
    const {
        translate,
        fromKg,
        getWeightUnit,
        formatDate,
        formatNumber,
        formatPercent,
    } = useContext(I18nSettingsContext);

    if (!active || !payload || payload.length === 0 || label == null) {
        return null;
    }
    const data = payload[0].payload as ChartDataType[number];
    return (
        <div className="chart-tooltip">
            <div>
                <b>{formatDate(data.date / 1000)}</b>
            </div>
            <div>
                {translate("fat_ratio")}: {formatPercent(data.fatAvg7 / 100, 1)}
            </div>
            <div>
                {translate("weight")}:{" "}
                {formatNumber(fromKg(data.weightAvg7), 1)} {getWeightUnit()}
            </div>
            <div>
                {translate("muscle_mass")}:{" "}
                {formatNumber(fromKg(data.muscleAvg7), 1)} {getWeightUnit()}
            </div>
        </div>
    );
}

const DAY_MS = 24 * 60 * 60 * 1000;
const SEVEN_DAYS_MS = 7 * DAY_MS;

function toDayStart(ms: number): number {
    const d = new Date(ms);
    d.setHours(0, 0, 0, 0);
    return d.getTime();
}

type ChartModel = {
    chartData: ChartDataType;
    minDate: number;
    maxDate: number;
    minWeight: number;
    maxWeight: number;
    minMuscle: number;
    maxMuscle: number;
    minFat: number;
    maxFat: number;
};

const EMPTY_CHART: ChartModel = {
    chartData: [],
    minDate: 99999,
    maxDate: 0,
    minWeight: 0,
    maxWeight: 99999,
    minMuscle: 0,
    maxMuscle: 99999,
    minFat: 0,
    maxFat: 100,
};

function buildChartModel(metrics: BodyMetricListItem[]): ChartModel {
    const data_c = [...metrics];
    data_c.reverse();

    if (data_c.length === 0) {
        return EMPTY_CHART;
    }

    let lMinKg = 99999;
    let lMaxKg = 0;
    let lMinFat = 99999;
    let lMaxFat = 0;
    let lMinMuscle = 99999;
    let lMaxMuscle = 0;

    type DailyValue = {
        fat: number;
        muscle: number;
        weight: number;
        count: number;
    };
    const dailyValues = new Map<number, DailyValue>();
    data_c.forEach((data) => {
        lMinKg = Math.min(lMinKg, data.weight);
        lMaxKg = Math.max(lMaxKg, data.weight);
        lMinMuscle = Math.min(lMinMuscle, data.muscle_mass);
        lMaxMuscle = Math.max(lMaxMuscle, data.muscle_mass);
        lMinFat = Math.min(lMinFat, data.fat_ratio);
        lMaxFat = Math.max(lMaxFat, data.fat_ratio);

        const day = toDayStart(data.date * 1000);
        const entry = dailyValues.get(day) ?? {
            fat: 0,
            muscle: 0,
            weight: 0,
            count: 0,
        };
        entry.fat += data.fat_ratio;
        entry.muscle += data.muscle_mass;
        entry.weight += data.weight;
        entry.count += 1;
        dailyValues.set(day, entry);
    });

    // Fill gaps between logged days by linearly interpolating between the
    // nearest known days, so every calendar day has a value.
    const knownDays = [...dailyValues.keys()].sort((a, b) => a - b);
    const average = (entry: DailyValue) => ({
        fat: entry.fat / entry.count,
        muscle: entry.muscle / entry.count,
        weight: entry.weight / entry.count,
    });
    const interpolate = (a: number, b: number, frac: number) =>
        a + (b - a) * frac;

    const daily: {
        day: number;
        fat: number;
        muscle: number;
        weight: number;
    }[] = [];
    let k = 0;
    for (
        let day = knownDays[0];
        day <= knownDays[knownDays.length - 1];
        day += DAY_MS
    ) {
        while (k + 1 < knownDays.length && knownDays[k + 1] <= day) {
            k++;
        }
        if (knownDays[k] === day) {
            daily.push({ day, ...average(dailyValues.get(day)!) });
        } else {
            const prev = average(dailyValues.get(knownDays[k])!);
            const next = average(dailyValues.get(knownDays[k + 1])!);
            const frac =
                (day - knownDays[k]) / (knownDays[k + 1] - knownDays[k]);
            daily.push({
                day,
                fat: interpolate(prev.fat, next.fat, frac),
                muscle: interpolate(prev.muscle, next.muscle, frac),
                weight: interpolate(prev.weight, next.weight, frac),
            });
        }
    }

    const newChartData: ChartDataType = [];
    let windowStart = 0;
    for (let i = 0; i < daily.length; i++) {
        const entry = daily[i];
        while (daily[windowStart].day <= entry.day - SEVEN_DAYS_MS) {
            windowStart++;
        }
        let fatSum = 0;
        let muscleSum = 0;
        let weightSum = 0;
        const windowSize = i - windowStart + 1;
        for (let j = windowStart; j <= i; j++) {
            fatSum += daily[j].fat;
            muscleSum += daily[j].muscle;
            weightSum += daily[j].weight;
        }

        newChartData.push({
            date: entry.day,
            fatAvg7: fatSum / windowSize,
            muscleAvg7: muscleSum / windowSize,
            weightAvg7: weightSum / windowSize,
        });
    }

    const dates = newChartData.map(({ date }) => date);
    return {
        chartData: newChartData,
        minDate: Math.min(...dates),
        maxDate: Math.max(...dates),
        minWeight: lMinKg,
        maxWeight: lMaxKg,
        minMuscle: lMinMuscle,
        maxMuscle: lMaxMuscle,
        minFat: lMinFat,
        maxFat: lMaxFat,
    };
}

type Props = {
    metrics: BodyMetricListItem[];
};

export function BodyMetricsChart({ metrics }: Props) {
    const { translate } = useContext(I18nSettingsContext);

    const {
        chartData,
        minDate,
        maxDate,
        minWeight,
        maxWeight,
        minMuscle,
        maxMuscle,
        minFat,
        maxFat,
    } = buildChartModel(metrics);

    return (
        <>
            <div style={{ textAlign: "center" }}>
                <h4>{translate("7_days_avg")}</h4>
            </div>
            <div
                className="chart-container"
                style={{ position: "relative", top: "-40px" }}
            >
                <ResponsiveContainer width="100%" height="100%">
                    <LineChart
                        data={chartData}
                        margin={{ top: 5, right: 15, left: 5, bottom: 5 }}
                    >
                        <CartesianGrid
                            stroke="#80808000"
                            strokeDasharray="5 5"
                        />
                        <XAxis
                            dataKey="date"
                            type="number"
                            domain={[minDate, maxDate]}
                            stroke="#fff"
                            tick={false}
                            height={0}
                        />
                        <YAxis
                            yAxisId="fat"
                            stroke="#fff"
                            width={0}
                            domain={[minFat, maxFat]}
                            tick={false}
                        />
                        <YAxis
                            yAxisId="weight"
                            stroke="#fff"
                            width={0}
                            domain={[minWeight, maxWeight]}
                            tick={false}
                        />
                        <YAxis
                            yAxisId="muscle"
                            stroke="#fff"
                            width={0}
                            domain={[minMuscle, maxMuscle]}
                            tick={false}
                        />
                        <Line
                            yAxisId="weight"
                            name={translate("weight")}
                            type="monotone"
                            dataKey="weightAvg7"
                            stroke="cyan"
                            dot={false}
                            activeDot={false}
                            isAnimationActive={false}
                        />
                        <Line
                            yAxisId="muscle"
                            name={translate("muscle_mass")}
                            type="monotone"
                            dataKey="muscleAvg7"
                            stroke="green"
                            dot={false}
                            activeDot={false}
                            isAnimationActive={false}
                        />
                        <Line
                            yAxisId="fat"
                            name={translate("fat_ratio")}
                            type="monotone"
                            dataKey="fatAvg7"
                            stroke="#f00"
                            dot={false}
                            activeDot={false}
                            isAnimationActive={false}
                        />
                        <Tooltip
                            isAnimationActive={false}
                            content={(props) => <ChartTooltip {...props} />}
                        />
                        <Legend />
                    </LineChart>
                </ResponsiveContainer>
            </div>
        </>
    );
}
