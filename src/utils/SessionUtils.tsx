import { Marker } from "react-leaflet";
import { SessionDetails, SessionListItem, SessionSet } from "./backend/models";
import L from "leaflet";

const SEMICIRCLE_TO_DEGREES = 180.0 / 2 ** 31;

const startOfDay = (d: Date) =>
    new Date(d.getFullYear(), d.getMonth(), d.getDate());

export interface WorkoutLoad {
    date: number;
    upper: number;
    current: number;
    reference: number;
    lower: number;
}

export interface SessionFrontDetails extends SessionDetails {
    zones_times: number[];
    gps_segments: {
        coordinates: [[number, number], [number, number]];
        color: string;
    }[];
    valid_points: [number, number][];
    start_point: [number, number];
    finish_point: [number, number];
    hrRanges: [number, number, number];
    hrBreathData: {
        idx: number;
        hr: number;
        avg: number;
        color: string;
    }[];
    elevations: [number, number] | undefined;
    volume: number;
}

export class SessionUtils {
    public static detailsFromBackend(
        backDetails: SessionDetails,
        fromKg: (weight: number) => number,
    ): SessionFrontDetails {
        // Deep copy: the handlers below convert sets, coordinates and laps in place
        const details: SessionFrontDetails = {
            ...structuredClone(backDetails),
            zones_times: [0, 0, 0, 0, 0],
            gps_segments: [],
            valid_points: [],
            start_point: [0, 0],
            finish_point: [0, 0],
            hrBreathData: [],
            hrRanges: [0, 0, 0],
            volume: 0,
            elevations: undefined,
        };

        SessionUtils.handleSeries(details, fromKg);
        SessionUtils.handleAltitude(details);
        SessionUtils.handleLaps(details);
        SessionUtils.handleGpsCoordiates(details);
        SessionUtils.handleHeartRate(details);

        return details;
    }

    private static handleAltitude(details: SessionFrontDetails) {
        if (details.altitudes && details.altitudes.length > 0) {
            let asc = 0;
            let desc = 0;

            const filtered = details.altitudes.filter((v) => Boolean(v));
            for (let i = 1; i < filtered.length; i++) {
                if (filtered![i]! > filtered![i - 1]!) {
                    asc += filtered![i]! - filtered![i - 1]!;
                } else if (filtered![i]! < filtered![i - 1]!) {
                    desc += filtered![i - 1]! - filtered![i]!;
                }
            }

            details.elevations = [asc, desc];
        }
    }

    private static handleSeries(
        details: SessionFrontDetails,
        fromKg: (weight: number) => number,
    ) {
        if (details.sets) {
            details.sets.forEach((_, idx) => {
                const copy = { ...details.sets[idx] };
                copy.weight = Number(fromKg(copy.weight).toFixed(1));
                details.sets[idx] = copy;
                details.volume += copy.reps * copy.weight;
            });
        }
    }

    private static handleGpsCoordiates(details: SessionFrontDetails) {
        if (details.coordinates) {
            for (let i = 0; i < details.coordinates.length; i++) {
                if (details.coordinates[i]) {
                    details.start_point = details.coordinates[i]!;
                    break;
                }
            }

            for (let i = details.coordinates.length - 1; i >= 0; i--) {
                if (details.coordinates[i]) {
                    details.finish_point = details.coordinates[i]!;
                    break;
                }
            }

            for (let i = 0; i < details.coordinates.length; i++) {
                if (details.coordinates[i]) {
                    details.coordinates[i]![0] =
                        details.coordinates[i]![0] * SEMICIRCLE_TO_DEGREES;
                    details.coordinates[i]![1] =
                        details.coordinates[i]![1] * SEMICIRCLE_TO_DEGREES;
                    details.valid_points.push(details.coordinates[i]!);
                }
            }

            const colors = [];
            for (let i = 0; i < details.coordinates.length - 1; i++) {
                colors.push(240);
            }
            if (details.speeds && details.speeds.length > 0) {
                const speeds: number[] = [];
                for (let i = 0; i < details.speeds.length; i++) {
                    if (details.speeds[i]) {
                        speeds.push(details.speeds[i]!);
                    }
                }
                const sortedSpeeds = [...speeds].sort((a, b) => a - b);
                const total = sortedSpeeds.length;

                const lowerBound = (value: number) => {
                    let lo = 0;
                    let hi = sortedSpeeds.length;
                    while (lo < hi) {
                        const mid = (lo + hi) >>> 1;
                        if (sortedSpeeds[mid] < value) {
                            lo = mid + 1;
                        } else {
                            hi = mid;
                        }
                    }
                    return lo;
                };
                const upperBound = (value: number) => {
                    let lo = 0;
                    let hi = sortedSpeeds.length;
                    while (lo < hi) {
                        const mid = (lo + hi) >>> 1;
                        if (sortedSpeeds[mid] <= value) {
                            lo = mid + 1;
                        } else {
                            hi = mid;
                        }
                    }
                    return lo;
                };

                for (let i = 0; i < speeds.length && i < colors.length; i++) {
                    const countLess = lowerBound(speeds[i]);
                    const countEqual = upperBound(speeds[i]) - countLess;
                    const percentile = (countLess + countEqual / 2) / total;
                    colors[i] = 240 - Math.round(240 * percentile);
                }
            }

            for (let i = 0; i < details.coordinates.length - 1; i++) {
                if (details.coordinates[i]) {
                    details.gps_segments.push({
                        coordinates: [
                            details.coordinates[i]!,
                            details.coordinates[i + 1]!,
                        ],
                        color: `hsl(${colors[i]}, 100%, 50%)`,
                    });
                }
            }
        }
    }

    private static handleLaps(details: SessionFrontDetails) {
        if (details.laps.length > 0) {
            if (details.laps.length > 0) {
                for (let i = 0; i < details.laps.length; i++) {
                    if (
                        details.laps[i].start_latitude &&
                        details.laps[i].start_longitude
                    ) {
                        details.laps[i].start_latitude =
                            details.laps[i].start_latitude! *
                            SEMICIRCLE_TO_DEGREES;
                        details.laps[i].start_longitude =
                            details.laps[i].start_longitude! *
                            SEMICIRCLE_TO_DEGREES;
                    }
                }
            }
        }
    }

    private static handleHeartRate(details: SessionFrontDetails) {
        // Without any sample (no HR sensor) keep the defaults so the HR section stays hidden
        if (details.heart_rates?.some((hr) => hr)) {
            const validHrs = [];
            for (let i = 0; i < details.heart_rates.length; i++) {
                if (details.heart_rates[i]) {
                    validHrs.push(details.heart_rates[i]!);
                } else {
                    validHrs.push(0);
                }
            }

            const maxHr = Math.max(details.max_hr, ...validHrs);

            details.zones_times = [0, 0, 0, 0, 0];
            validHrs.forEach((hr) => {
                const rate = hr / maxHr;
                if (rate < 0.6) {
                    details.zones_times[0]++;
                } else if (rate < 0.7) {
                    details.zones_times[1]++;
                } else if (rate < 0.8) {
                    details.zones_times[2]++;
                } else if (rate < 0.9) {
                    details.zones_times[3]++;
                } else {
                    details.zones_times[4]++;
                }
            });

            const timeFraction =
                details.total_elapsed_time / details.heart_rates.length;
            let accum = 0;
            details.zones_times.forEach((count, idx) => {
                const time_i = Math.round(count * timeFraction);
                details.zones_times[idx] = time_i;
                accum += time_i;
            });

            details.zones_times[0] +=
                Math.round(details.total_elapsed_time) - accum;

            if (validHrs.length > 0) {
                const nonZeroHrs = validHrs.filter((valor) => valor !== 0);

                details.hrRanges = [
                    Math.min(...nonZeroHrs),
                    Math.round(
                        nonZeroHrs.reduce((acc, valor) => acc + valor, 0) /
                            nonZeroHrs.length,
                    ),
                    Math.max(...nonZeroHrs),
                ];

                validHrs.forEach((hr, idx) => {
                    let color = "red";

                    const rateVal = (hr * 1.0) / (maxHr * 1.0);
                    if (rateVal <= 0.6) {
                        color = "gray";
                    } else if (rateVal <= 0.7) {
                        color = "turquoise";
                    } else if (rateVal <= 0.8) {
                        color = "green";
                    } else if (rateVal <= 0.9) {
                        color = "orange";
                    }

                    details.hrBreathData.push({
                        idx: idx,
                        hr,
                        avg: details.hrRanges[1],
                        color,
                    });
                });
            }
        }
    }

    public static CHRONIC_DAYS = 28;
    public static ACWR_UPPER_RATIO = 1.4;

    public static isOverreaching(load: WorkoutLoad[]): boolean {
        if (load.length === 0) {
            return false;
        }
        const last = load[load.length - 1];
        return last.current > last.reference * SessionUtils.ACWR_UPPER_RATIO;
    }

    public static calculateWorkoutLoad(data: SessionListItem[]): WorkoutLoad[] {
        if (data.length === 0) {
            return [];
        } else {
            const addDays = (ts: number, n: number) => {
                const d = new Date(ts);
                d.setDate(d.getDate() + n);
                return d.getTime();
            };

            const ACUTE_DAYS = 7;
            const ACWR_LOWER_RATIO = 0.9;
            const TODAY = startOfDay(new Date()).getTime();

            const LAMBDA_ACUTE = 2 / (ACUTE_DAYS + 1); // ~0.25
            const LAMBDA_CHRONIC = 2 / (SessionUtils.CHRONIC_DAYS + 1); // ~0.069

            let working_data = Array.from(
                data
                    .map((s) => {
                        return {
                            date: startOfDay(
                                new Date(s.timestamp * 1000),
                            ).getTime(),
                            load: s.training_load,
                        };
                    })
                    .filter(
                        (s) =>
                            TODAY -
                                2 *
                                    SessionUtils.CHRONIC_DAYS *
                                    24 *
                                    60 *
                                    60 *
                                    1000 <=
                            s.date,
                    )
                    .reduce((map, s) => {
                        map.set(s.date, (map.get(s.date) ?? 0) + s.load);
                        return map;
                    }, new Map<number, number>()),
                ([date, load]) => ({ date, load }),
            );

            for (
                let dat = addDays(TODAY, -2 * SessionUtils.CHRONIC_DAYS + 1);
                dat <= TODAY;
                dat = addDays(dat, 1)
            ) {
                if (!working_data.find(({ date }) => date === dat)) {
                    working_data.push({ date: dat, load: 0 });
                }
            }

            working_data = working_data.sort((a, b) => a.date - b.date);

            if (working_data.length === 0) {
                return [];
            } else {
                let ewmaAcute = working_data[0].load;
                let ewmaChronic = working_data[0].load;

                const ewmaSeries: {
                    date: number;
                    acute: number;
                    chronic: number;
                }[] = [
                    {
                        date: working_data[0].date,
                        acute: ewmaAcute,
                        chronic: ewmaChronic,
                    },
                ];

                for (let i = 1; i < working_data.length; i++) {
                    const v = working_data[i].load;
                    ewmaAcute =
                        v * LAMBDA_ACUTE + ewmaAcute * (1 - LAMBDA_ACUTE);
                    ewmaChronic =
                        v * LAMBDA_CHRONIC + ewmaChronic * (1 - LAMBDA_CHRONIC);

                    ewmaSeries.push({
                        date: working_data[i].date,
                        acute: ewmaAcute,
                        chronic: ewmaChronic,
                    });
                }

                let load_data = ewmaSeries
                    .filter((_, idx) => idx >= SessionUtils.CHRONIC_DAYS)
                    .map((e) => ({
                        date: e.date,
                        upper:
                            e.chronic *
                            (SessionUtils.ACWR_UPPER_RATIO - ACWR_LOWER_RATIO),
                        lower: e.chronic * ACWR_LOWER_RATIO,
                        current: e.acute,
                        reference: e.chronic,
                    }));

                if (load_data.length === 0) {
                    return [];
                } else {
                    load_data = load_data.map((e) => ({
                        ...e,
                        current: e.current,
                        upper: e.upper,
                        lower: e.lower,
                        reference: e.reference,
                    }));

                    return load_data;
                }
            }
        }
    }

    private static makeMarkerIcon(
        color: string,
        symbol: "play" | "stop",
    ): L.DivIcon {
        return L.divIcon({
            className: "",
            html: `<svg width="25" height="41" viewBox="0 0 25 41" xmlns="http://www.w3.org/2000/svg">
        <path d="M12.5 0C5.6 0 0 5.6 0 12.5c0 9.4 12.5 28.5 12.5 28.5S25 21.9 25 12.5C25 5.6 19.4 0 12.5 0z" fill="${color}" stroke="white" stroke-width="1.5"/>
        <circle cx="12.5" cy="12.5" r="8" fill="white"/>
        ${
            symbol === "play"
                ? `<path d="M10.5 8.5L17.17 12.5L10.5 16.5Z" fill="${color}"/>`
                : `<rect x="8.5" y="8.5" width="8" height="8" fill="${color}"/>`
        }
      </svg>`,
            iconSize: [25, 41],
            iconAnchor: [12, 41],
            popupAnchor: [1, -34],
        });
    }

    public static START_ICON = SessionUtils.makeMarkerIcon("#249d56", "play");
    public static END_ICON = SessionUtils.makeMarkerIcon("#e74c3c", "stop");

    public static makeLapMarker(
        idx: number,
        lap: [number, number],
    ): React.JSX.Element {
        const lapColor = "#2A81CB";
        const icon = L.divIcon({
            className: "",
            html: `<svg width="25" height="41" viewBox="0 0 25 41" xmlns="http://www.w3.org/2000/svg">
          <path d="M12.5 0C5.6 0 0 5.6 0 12.5c0 9.4 12.5 28.5 12.5 28.5S25 21.9 25 12.5C25 5.6 19.4 0 12.5 0z" fill="${lapColor}" stroke="white" stroke-width="1.5"/>
          <circle cx="12.5" cy="12.5" r="8" fill="white"/>
          <text x="12.5" y="12.5" text-anchor="middle" dominant-baseline="central" font-size="10" font-weight="bold" fill="${lapColor}">${idx}</text>
        </svg>`,
            iconSize: [25, 41],
            iconAnchor: [12, 41],
            popupAnchor: [1, -34],
        });

        return <Marker key={idx} position={lap} icon={icon} />;
    }

    private static MAX_ESTIMATION_REPS = 20;

    public static estimate1rm(weight: number, reps: number): number {
        if (reps <= 1) {
            return Math.trunc(weight);
        }

        const r = Math.min(reps, SessionUtils.MAX_ESTIMATION_REPS);

        if (r <= 10) {
            return Math.trunc((weight * 36) / (37 - r));
        }
        return Math.trunc((100 * weight) / (101.3 - 2.67123 * r));
    }

    public static maxE1rm(sets: SessionSet[]): number {
        return sets.reduce(
            (max, s) =>
                Math.max(max, SessionUtils.estimate1rm(s.weight, s.reps)),
            0,
        );
    }
}
