//Auto generated file, do not edit manually

// From src-tauri/src/dto/app.rs:5
export enum AppEnvironment {
	Debug = "Debug",
	Release = "Release",
}

// From src-tauri/src/dto/body_metrics.rs:6
export interface BodyMetricListItem {
  date: number;
  fat_ratio: number;
  lean_mass: number;
  water_ratio: number;
  weight: number;
}

// From src-tauri/src/rclone/providers/mod.rs:11
export enum CloudProvider {
	OneDrive = "OneDrive",
	DropBox = "DropBox",
}

// From src-tauri/src/dto/devices.rs:7
export interface DeviceListItem {
  manufacturer: string;
  model: string;
  serial_number: string;
}

// From src-tauri/src/dao/settings.rs:200
export enum DistanceUnit {
	Kilometers = "Kilometers",
	Miles = "Miles",
}

// From src-tauri/src/dto/exercises.rs:33
export interface ExerciseDetails {
  category: number;
  id: number;
  pr_date: number;
  progression: boolean;
  reps: number;
  reps_range: string;
  series: Record<string, SessionSet[]>;
  unilateral: boolean;
  weight: number;
  workouts: string[];
}

// From src-tauri/src/dto/exercises.rs:8
export interface ExerciseListItem {
  category: number;
  date: number;
  id: number;
  progression: boolean;
  reps: number;
  unilateral: boolean;
  weight: number;
}

// From src-tauri/src/utils/translations.rs:10
export enum Languages {
	Spanish = "Spanish",
	English = "English",
}

// From src-tauri/src/dto/exercises.rs:64
export interface ProgressionSuggestion {
  reps: number;
  weight: number;
}

// From src-tauri/src/dto/report.rs:8
export interface ReportData {
  body_metrics: BodyMetricListItem[];
  exercises: ExerciseDetails[];
  sessions: SessionDetails[];
}

// From src-tauri/src/dto/sessions.rs:97
export interface SessionDetails {
  active_calories: number;
  active_time: number;
  altitudes: (number | null)[];
  coordinates: ([number, number] | null)[];
  device: string | null;
  distance: number | null;
  heart_rates: (number | null)[];
  laps: SessionLap[];
  max_hr: number;
  name: string;
  notes: string;
  sets: SessionSet[];
  speeds: (number | null)[];
  sport: number;
  sub_sport: number;
  timestamp: number;
  total_elapsed_time: number;
  training_load: number;
}

// From src-tauri/src/dto/sessions.rs:80
export interface SessionLap {
  idx: number;
  start_latitude: number | null;
  start_longitude: number | null;
}

// From src-tauri/src/dto/sessions.rs:13
export interface SessionListItem {
  active_calories: number;
  has_record: boolean;
  has_sets: boolean;
  name: string;
  sport: number;
  sub_sport: number;
  timestamp: number;
  total_elapsed_time: number;
  training_load: number;
}

// From src-tauri/src/dto/sessions.rs:204
export interface SessionLocation {
  location: string;
  session: number;
}

// From src-tauri/src/dto/sessions.rs:53
export interface SessionSet {
  duration: number;
  ex_cat: number;
  ex_id: number;
  idx: number;
  pr: boolean;
  reps: number;
  rest: boolean;
  weight: number;
}

// From src-tauri/src/dto/sessions.rs:197
export interface SessionSetsUpdate {
  notes: string | null;
  sets: SessionSet[];
  timestamp: number;
}

// From src-tauri/src/dto/app.rs:16
export interface Settings {
  close_to_tray: boolean;
  distance_unit: DistanceUnit;
  language: Languages;
  on_device_connect: boolean;
  start_boot: boolean;
  start_into_tray: boolean;
  weight_unit: WeightUnit;
}

// From src-tauri/src/dao/workout_step.rs:40
export enum StepType {
	Exercise = "Exercise",
	Rest = "Rest",
	Repeat = "Repeat",
}

// From src-tauri/src/dao/settings.rs:228
export enum WeightUnit {
	Kilograms = "Kilograms",
	Pounds = "Pounds",
}

// From src-tauri/src/dao/workout.rs:8
export interface Workout {
  enabled: boolean;
  name: string;
  notes: string | null;
  steps: WorkoutStep[];
}

// From src-tauri/src/dto/workouts.rs:34
export interface WorkoutDetails {
  avg_time: number;
  avg_volume: number;
  enabled: boolean;
  latest_session: number | null;
  name: string;
  notes: string | null;
  session_count: number;
  sessions: WorkoutSession[];
  steps: WorkoutStep[];
}

// From src-tauri/src/dto/workouts.rs:6
export interface WorkoutListItem {
  avg_load: number;
  avg_time: number;
  enabled: boolean;
  has_steps: boolean;
  latest_session: number | null;
  name: string;
  sessions: number;
}

// From src-tauri/src/dto/workouts.rs:17
export interface WorkoutSession {
  date: number;
  time: number;
  volume: number;
}

// From src-tauri/src/dao/workout_step.rs:16
export interface WorkoutStep {
  begin_idx: number | null;
  ex_cat: number | null;
  ex_id: number | null;
  idx: number;
  kind: StepType;
  notes: string | null;
  reps: number | null | null;
  time: number | null | null;
  weight: number | null;
  workout: string;
}

