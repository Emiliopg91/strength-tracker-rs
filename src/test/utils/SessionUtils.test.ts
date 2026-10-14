import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SessionUtils, WorkoutLoad } from "@/utils/SessionUtils";
import {
  SessionDetails,
  SessionListItem,
  SessionSet,
} from "@/utils/backend/models";
import { KG_TO_LB } from "@/context/I18nSettingsContext";

const kgToKg = (kg: number) => kg;
const kgToLb = (kg: number) => kg * KG_TO_LB;

/** Degrees expressed in Garmin semicircles. */
const semicircles = (degrees: number) => (degrees * 2 ** 31) / 180;

const makeDetails = (
  overrides: Partial<SessionDetails> = {},
): SessionDetails => ({
  active_time: 0,
  coordinates: [],
  device: null,
  distance: null,
  heart_rates: [],
  max_hr: 189,
  laps: [],
  name: "Session",
  notes: "",
  sets: [],
  speeds: [],
  altitudes: [],
  sport: 0,
  sub_sport: 0,
  timestamp: 0,
  active_calories: 0,
  total_elapsed_time: 0,
  training_load: 0,
  ...overrides,
});

const makeSet = (overrides: Partial<SessionSet> = {}): SessionSet => ({
  ex_cat: 0,
  ex_id: 0,
  idx: 0,
  pr: false,
  reps: 10,
  rest: false,
  weight: 50,
  duration: 0,
  ...overrides,
});

describe("SessionUtils.detailsFromBackend - sets", () => {
  it("computes volume across all sets", () => {
    const details = SessionUtils.detailsFromBackend(
      makeDetails({
        sets: [
          makeSet({ idx: 0, ex_cat: 1, ex_id: 2, reps: 10, weight: 60 }),
          makeSet({ idx: 1, ex_cat: 3, ex_id: 4, reps: 8, weight: 40 }),
          makeSet({ idx: 2, ex_cat: 1, ex_id: 2, reps: 5, weight: 70 }),
        ],
      }),
      kgToKg,
    );

    expect(details.volume).toBe(10 * 60 + 8 * 40 + 5 * 70);
  });

  it("converts weights to the selected unit rounded to one decimal", () => {
    const details = SessionUtils.detailsFromBackend(
      makeDetails({ sets: [makeSet({ reps: 2, weight: 100 })] }),
      kgToLb,
    );

    expect(details.sets[0].weight).toBe(220.5);
    expect(details.volume).toBeCloseTo(441, 6);
  });

  it("rounds kilograms to one decimal", () => {
    const details = SessionUtils.detailsFromBackend(
      makeDetails({ sets: [makeSet({ weight: 62.46 })] }),
      kgToKg,
    );

    expect(details.sets[0].weight).toBe(62.5);
  });

  it("leaves defaults when there are no sets", () => {
    const details = SessionUtils.detailsFromBackend(makeDetails(), kgToKg);

    expect(details.volume).toBe(0);
  });
});

describe("SessionUtils.detailsFromBackend - laps", () => {
  it("converts lap start positions from semicircles to degrees", () => {
    const details = SessionUtils.detailsFromBackend(
      makeDetails({
        laps: [
          {
            idx: 0,
            start_latitude: semicircles(40),
            start_longitude: semicircles(-3),
          },
          { idx: 1, start_latitude: null, start_longitude: null },
        ],
      }),
      kgToKg,
    );

    expect(details.laps[0].start_latitude).toBeCloseTo(40, 10);
    expect(details.laps[0].start_longitude).toBeCloseTo(-3, 10);
    expect(details.laps[1].start_latitude).toBeNull();
    expect(details.laps[1].start_longitude).toBeNull();
  });
});

describe("SessionUtils.detailsFromBackend - GPS", () => {
  it("finds start/finish points and skips missing coordinates", () => {
    const details = SessionUtils.detailsFromBackend(
      makeDetails({
        distance: 5,
        coordinates: [
          null,
          [semicircles(40), semicircles(-3)],
          [semicircles(41), semicircles(-3)],
          [semicircles(42), semicircles(-4)],
          null,
        ],
      }),
      kgToKg,
    );

    expect(details.start_point[0]).toBeCloseTo(40, 10);
    expect(details.start_point[1]).toBeCloseTo(-3, 10);
    expect(details.finish_point[0]).toBeCloseTo(42, 10);
    expect(details.finish_point[1]).toBeCloseTo(-4, 10);
    expect(details.valid_points).toHaveLength(3);
  });

  it("keeps the distance reported by the device", () => {
    const details = SessionUtils.detailsFromBackend(
      makeDetails({
        distance: 5,
        coordinates: [
          [semicircles(0), semicircles(0)],
          [semicircles(1), semicircles(0)],
        ],
      }),
      kgToKg,
    );

    expect(details.distance).toBe(5);
  });

  it("paints every segment blue when there is no speed data", () => {
    const details = SessionUtils.detailsFromBackend(
      makeDetails({
        distance: 1,
        coordinates: [
          [semicircles(0), semicircles(0)],
          [semicircles(1), semicircles(0)],
          [semicircles(2), semicircles(0)],
        ],
      }),
      kgToKg,
    );

    expect(details.gps_segments).toHaveLength(2);
    expect(details.gps_segments.map((s) => s.color)).toEqual([
      "hsl(240, 100%, 50%)",
      "hsl(240, 100%, 50%)",
    ]);
    expect(details.gps_segments[1].coordinates[0][0]).toBeCloseTo(1, 10);
    expect(details.gps_segments[1].coordinates[1][0]).toBeCloseTo(2, 10);
  });

  it("colors segments by speed percentile, from blue (slow) to red (fast)", () => {
    const details = SessionUtils.detailsFromBackend(
      makeDetails({
        distance: 1,
        coordinates: [
          [semicircles(0), semicircles(0)],
          [semicircles(1), semicircles(0)],
          [semicircles(2), semicircles(0)],
          [semicircles(3), semicircles(0)],
        ],
        speeds: [1, 2, 3],
      }),
      kgToKg,
    );

    expect(details.gps_segments.map((s) => s.color)).toEqual([
      "hsl(200, 100%, 50%)",
      "hsl(120, 100%, 50%)",
      "hsl(40, 100%, 50%)",
    ]);
  });
});

describe("SessionUtils.detailsFromBackend - heart rate", () => {
  it("distributes elapsed time across zones using a 189 bpm floor for max HR", () => {
    const details = SessionUtils.detailsFromBackend(
      makeDetails({
        total_elapsed_time: 100,
        heart_rates: [100, 150, 180, null],
      }),
      kgToKg,
    );

    // 100 & missing -> Z1, 150 (79%) -> Z3, 180 (95%) -> Z5
    expect(details.zones_times).toEqual([50, 0, 25, 0, 25]);
  });

  it("uses the observed max HR when it is above 189", () => {
    const details = SessionUtils.detailsFromBackend(
      makeDetails({ total_elapsed_time: 20, heart_rates: [200, 100] }),
      kgToKg,
    );

    expect(details.zones_times).toEqual([10, 0, 0, 0, 10]);
    expect(details.hrBreathData.map((d) => d.color)).toEqual(["red", "gray"]);
  });

  it("adjusts the first zone so zones always add up to the elapsed time", () => {
    const details = SessionUtils.detailsFromBackend(
      makeDetails({
        total_elapsed_time: 10,
        heart_rates: [100, 120, 150, 160],
      }),
      kgToKg,
    );

    // Each zone rounds 2.5 up to 3, so Z1 absorbs the -2 difference
    expect(details.zones_times).toEqual([1, 3, 3, 3, 0]);
    expect(details.zones_times.reduce((a, b) => a + b, 0)).toBe(10);
  });

  it("computes min/avg/max ignoring missing samples", () => {
    const details = SessionUtils.detailsFromBackend(
      makeDetails({
        total_elapsed_time: 4,
        heart_rates: [100, null, 150, 180],
      }),
      kgToKg,
    );

    expect(details.hrRanges).toEqual([100, 143, 180]);
  });

  it.each([
    ["no samples", []],
    ["only missing samples", [null, null, null]],
  ])(
    "keeps HR defaults with %s",
    (_: unknown, heart_rates: (number | null)[]) => {
      const details = SessionUtils.detailsFromBackend(
        makeDetails({ total_elapsed_time: 30, heart_rates }),
        kgToKg,
      );

      expect(details.zones_times).toEqual([0, 0, 0, 0, 0]);
      expect(details.hrRanges).toEqual([0, 0, 0]);
      expect(details.hrBreathData).toEqual([]);
    },
  );

  it("builds per-sample chart data colored by zone", () => {
    const details = SessionUtils.detailsFromBackend(
      makeDetails({
        total_elapsed_time: 5,
        heart_rates: [100, 125, 150, 165, 180],
      }),
      kgToKg,
    );

    expect(details.hrBreathData).toEqual([
      { idx: 0, hr: 100, avg: 144, color: "gray" },
      { idx: 1, hr: 125, avg: 144, color: "turquoise" },
      { idx: 2, hr: 150, avg: 144, color: "green" },
      { idx: 3, hr: 165, avg: 144, color: "orange" },
      { idx: 4, hr: 180, avg: 144, color: "red" },
    ]);
  });
});

describe("SessionUtils.detailsFromBackend - input", () => {
  const backDetails = () =>
    makeDetails({
      sets: [makeSet({ weight: 100 })],
      coordinates: [
        [semicircles(40), semicircles(-3)],
        [semicircles(41), semicircles(-3)],
      ],
      laps: [
        {
          idx: 0,
          start_latitude: semicircles(40),
          start_longitude: semicircles(-3),
        },
      ],
      total_elapsed_time: 2,
      heart_rates: [120, 150],
    });

  it("does not modify the backend details", () => {
    const back = backDetails();
    const snapshot = structuredClone(back);

    SessionUtils.detailsFromBackend(back, kgToLb);

    expect(back).toEqual(snapshot);
  });

  it("gives the same result when called twice with the same details", () => {
    const back = backDetails();

    const first = SessionUtils.detailsFromBackend(back, kgToLb);
    const second = SessionUtils.detailsFromBackend(back, kgToLb);

    expect(second).toEqual(first);
  });
});

describe("SessionUtils.isOverreaching", () => {
  const load = (current: number, reference: number): WorkoutLoad => ({
    date: 0,
    upper: 0,
    lower: 0,
    current,
    reference,
  });

  it("is false without data", () => {
    expect(SessionUtils.isOverreaching([])).toBe(false);
  });

  it("only looks at the last entry", () => {
    expect(SessionUtils.isOverreaching([load(1000, 1), load(100, 100)])).toBe(
      false,
    );
    expect(SessionUtils.isOverreaching([load(100, 100), load(141, 100)])).toBe(
      true,
    );
  });

  it("uses the ACWR upper ratio as a strict threshold", () => {
    expect(SessionUtils.isOverreaching([load(140, 100)])).toBe(false);
  });
});

describe("SessionUtils.calculateWorkoutLoad", () => {
  const DAY_MS = 24 * 60 * 60 * 1000;
  const NOW = new Date(2025, 5, 15, 18, 30);
  const TODAY = new Date(2025, 5, 15).getTime();

  const session = (daysAgo: number, training_load: number): SessionListItem => {
    const date = new Date(NOW);
    date.setDate(date.getDate() - daysAgo);
    return {
      active_calories: 0,
      has_record: false,
      name: "Session",
      sport: 0,
      sub_sport: 0,
      timestamp: date.getTime() / 1000,
      total_elapsed_time: 0,
      training_load,
      has_sets: false,
    };
  };

  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(NOW);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("returns nothing without sessions", () => {
    expect(SessionUtils.calculateWorkoutLoad([])).toEqual([]);
  });

  it("returns one entry per day for the last chronic window, ending today", () => {
    const result = SessionUtils.calculateWorkoutLoad([session(0, 100)]);

    expect(result).toHaveLength(SessionUtils.CHRONIC_DAYS);
    expect(result[result.length - 1].date).toBe(TODAY);
    for (let i = 1; i < result.length; i++) {
      // Allow for DST shifts
      expect(
        Math.abs(result[i].date - result[i - 1].date - DAY_MS),
      ).toBeLessThanOrEqual(60 * 60 * 1000);
    }
  });

  it("applies EWMA to a single load spike", () => {
    const result = SessionUtils.calculateWorkoutLoad([session(0, 100)]);
    const last = result[result.length - 1];

    const chronic = (100 * 2) / (SessionUtils.CHRONIC_DAYS + 1);
    expect(last.current).toBeCloseTo(25, 10);
    expect(last.reference).toBeCloseTo(chronic, 10);
    expect(last.lower).toBeCloseTo(chronic * 0.9, 10);
    expect(last.upper).toBeCloseTo(chronic * 0.5, 10);
    expect(
      result.slice(0, -1).every((e) => e.current === 0 && e.reference === 0),
    ).toBe(true);
    expect(SessionUtils.isOverreaching(result)).toBe(true);
  });

  it("sums sessions on the same day", () => {
    const single = SessionUtils.calculateWorkoutLoad([session(0, 100)]);
    const split = SessionUtils.calculateWorkoutLoad([
      session(0, 60),
      session(0, 40),
    ]);

    expect(split).toEqual(single);
  });

  it("stays flat with a constant daily load", () => {
    const sessions = Array.from(
      { length: 2 * SessionUtils.CHRONIC_DAYS },
      (_, i) => session(i, 80),
    );
    const result = SessionUtils.calculateWorkoutLoad(sessions);

    for (const entry of result) {
      expect(entry.current).toBeCloseTo(80, 10);
      expect(entry.reference).toBeCloseTo(80, 10);
    }
    expect(SessionUtils.isOverreaching(result)).toBe(false);
  });

  it("ignores sessions older than twice the chronic window", () => {
    const result = SessionUtils.calculateWorkoutLoad([
      session(2 * SessionUtils.CHRONIC_DAYS + 10, 500),
    ]);

    expect(result).toHaveLength(SessionUtils.CHRONIC_DAYS);
    expect(result.every((e) => e.current === 0 && e.reference === 0)).toBe(
      true,
    );
  });
});

describe("SessionUtils.maxE1rm", () => {
  it("returns 0 for no sets", () => {
    expect(SessionUtils.maxE1rm([])).toBe(0);
  });

  it("returns the highest estimate across sets", () => {
    const sets = [
      makeSet({ weight: 100, reps: 1 }),
      makeSet({ weight: 90, reps: 5 }),
      makeSet({ weight: 60, reps: 12 }),
    ];

    expect(SessionUtils.maxE1rm(sets)).toBe(
      Math.max(
        SessionUtils.estimate1rm(100, 1),
        SessionUtils.estimate1rm(90, 5),
        SessionUtils.estimate1rm(60, 12),
      ),
    );
    expect(SessionUtils.maxE1rm(sets)).toBe(101);
  });
});
