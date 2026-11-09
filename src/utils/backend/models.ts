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

// From src-tauri/src/dto/devices.rs:6
export interface DeviceListItem {
  manufacturer: string;
  model: string;
  serial_number: string;
  strength_workout: boolean;
}

// From src-tauri/src/dao/settings.rs:200
export enum DistanceUnit {
	Kilometers = "Kilometers",
	Miles = "Miles",
}

// From src-tauri/src/dao/exercise.rs:12
export interface Exercise {
  category: number;
  exercise_category: ExerciseCategory | null;
  id: number;
  muscles: MuscleExercise[];
  name: string;
  notes: string | null;
  progression: boolean;
  reps: string;
  unilateral: boolean;
  weight: number;
}

// From src-tauri/src/dao/exercise_category.rs:7
export interface ExerciseCategory {
  id: number;
  name: string;
}

// From src-tauri/src/dto/exercises.rs:41
export interface ExerciseDetails {
  category: number;
  id: number;
  notes: string | null;
  pr_date: number;
  primary_muscles: string[];
  prog_weight: number;
  progression: boolean;
  reps: number;
  reps_range: string;
  secondary_muscles: string[];
  sessions: Record<string, ExerciseSession>;
  unilateral: boolean;
  weight: number;
  workouts: string[];
}

// From src-tauri/src/dto/exercises.rs:8
export interface ExerciseListItem {
  category: number;
  date: number;
  id: number;
  notes: string | null;
  progression: boolean;
  reps: number;
  unilateral: boolean;
  weight: number;
}

// From src-tauri/src/dto/exercises.rs:35
export interface ExerciseSession {
  series: SessionStepDto[];
  weight: number | null;
}

// From src-tauri/src/utils/translations.rs:10
export enum Languages {
	Spanish = "Spanish",
	English = "English",
}

// From src-tauri/src/dao/muscle.rs:5
export interface Muscle {
  name: string;
}

// From src-tauri/src/dao/muscle_exercise.rs:8
export interface MuscleExercise {
  ex_cat: number;
  ex_id: number;
  is_primary: boolean;
  muscle: string;
  muscle_obj: Muscle | null;
}

// From src-tauri/src/dto/exercises.rs:100
export interface ProgressionSuggestion {
  reps: number;
  weight: number;
}

// From src-tauri/src/dto/prompts.rs:5
export enum PromptKind {
	ConnectCredentials = "ConnectCredentials",
	ConnectMfa = "ConnectMfa",
	SudoPassword = "SudoPassword",
}

// From src-tauri/src/dto/prompts.rs:17
export interface PromptRequest {
  detail: string | null;
  id: number;
  kind: PromptKind;
  prefill: string | null;
}

// From src-tauri/src/dto/report.rs:8
export interface ReportData {
  body_metrics: BodyMetricListItem[];
  exercises: ExerciseDetails[];
  sessions: SessionDetails[];
}

// From src-tauri/src/dto/sessions.rs:101
export interface SessionDetails {
  active_calories: number;
  active_time: number;
  coordinates: ([number, number] | null)[];
  device: string | null;
  distance: number | null;
  heart_rates: (number | null)[];
  laps: SessionLapDto[];
  max_hr: number;
  name: string;
  notes: string;
  primary_muscles: string[];
  secondary_muscles: string[];
  speeds: (number | null)[];
  sport: number;
  steps: SessionStepDto[];
  sub_sport: number;
  timestamp: number;
  total_elapsed_time: number;
  training_load: number;
}

// From src-tauri/src/dto/sessions.rs:84
export interface SessionLapDto {
  idx: number;
  start_latitude: number | null;
  start_longitude: number | null;
}

// From src-tauri/src/dto/sessions.rs:11
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

// From src-tauri/src/dto/sessions.rs:225
export interface SessionLocation {
  location: string;
  session: number;
}

// From src-tauri/src/dto/sessions.rs:51
export interface SessionStepDto {
  duration: number | null;
  duration_ref: number | null;
  ex_cat: number | null;
  ex_id: number | null;
  idx: number;
  kind: StepType;
  pr: boolean;
  reps: number | null;
  reps_ref: number | null;
  weight: number | null;
  weight_ref: number | null;
}

// From src-tauri/src/dto/sessions.rs:218
export interface SessionStepsUpdate {
  notes: string | null;
  sets: SessionStepDto[];
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

// From src-tauri/src/dao/workout_step.rs:47
export enum StepType {
	Exercise = "Exercise",
	Warmup = "Warmup",
	Recovery = "Recovery",
	Cooldown = "Cooldown",
	Rest = "Rest",
	Other = "Other",
	Repeat = "Repeat",
}

// From src-tauri/src/dao/settings.rs:228
export enum WeightUnit {
	Kilograms = "Kilograms",
	Pounds = "Pounds",
}

// From src-tauri/src/dao/workout.rs:8
export interface Workout {
  connect_id: string | null;
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
  primary_muscles: string[];
  secondary_muscles: string[];
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

// From src-tauri/src/dao/workout_step.rs:19
export interface WorkoutStep {
  begin_idx: number | null;
  ex_cat: number | null;
  ex_id: number | null;
  exercise: Exercise | null;
  idx: number;
  kind: StepType;
  notes: string | null;
  reps: number | null | null;
  time: number | null | null;
  weight: number | null;
  workout: string;
}

