//Auto generated file, do not edit manually

import { invoke, InvokeArgs } from "@tauri-apps/api/core";

import { AppEnvironment, BodyMetricListItem, CloudProvider, DeviceListItem, ExerciseDetails, ExerciseListItem, ProgressionSuggestion, ReportData, SessionDetails, SessionListItem, SessionStepsUpdate, Settings, Workout, WorkoutDetails, WorkoutListItem } from "./models";

export class BackendClient {

    private static DONT_LOG_COMMANDS: string[] = ["answer_prompt"];

	// From src-tauri/src/logic/body_metrics/commands.rs:21
	public static addBodyMeasures(measures: BodyMetricListItem): Promise<void> {
	  return BackendClient.inner_invoke("add_body_measures", { measures }); 
	}
	

	// From src-tauri/src/logic/prompts/commands.rs:11
	public static answerPrompt(id: number, values: string[] | null): Promise<void> {
	  return BackendClient.inner_invoke("answer_prompt", { id, values }); 
	}
	

	// From src-tauri/src/logic/exercises/commands.rs:123
	public static applyProgressionSuggestion(category: number, id: number, suggestion: ProgressionSuggestion): Promise<void> {
	  return BackendClient.inner_invoke("apply_progression_suggestion", { category, id, suggestion }); 
	}
	

	// From src-tauri/src/logic/exercises/commands.rs:113
	public static availableAnyProgressionSuggestions(): Promise<boolean> {
	  return BackendClient.inner_invoke("available_any_progression_suggestions"); 
	}
	

	// From src-tauri/src/logic/connect/commands.rs:36
	public static connectLoggedUser(): Promise<string | null> {
	  return BackendClient.inner_invoke("connect_logged_user"); 
	}
	

	// From src-tauri/src/logic/connect/commands.rs:47
	public static connectLogin(): Promise<string | null> {
	  return BackendClient.inner_invoke("connect_login"); 
	}
	

	// From src-tauri/src/logic/connect/commands.rs:57
	public static connectLogout(): Promise<void> {
	  return BackendClient.inner_invoke("connect_logout"); 
	}
	

	// From src-tauri/src/logic/body_metrics/commands.rs:33
	public static deleteBodyMetric(date: number): Promise<void> {
	  return BackendClient.inner_invoke("delete_body_metric", { date }); 
	}
	

	// From src-tauri/src/logic/sessions/commands.rs:42
	public static downloadSession(timestamp: number): Promise<void> {
	  return BackendClient.inner_invoke("download_session", { timestamp }); 
	}
	

	// From src-tauri/src/logic/export/commands.rs:23
	public static exportDatabase(): Promise<void> {
	  return BackendClient.inner_invoke("export_database"); 
	}
	

	// From src-tauri/src/logic/body_metrics/commands.rs:10
	public static getBodyMeasures(): Promise<BodyMetricListItem[]> {
	  return BackendClient.inner_invoke("get_body_measures"); 
	}
	

	// From src-tauri/src/logic/app/commands.rs:31
	public static getEnvironment(): Promise<AppEnvironment> {
	  return BackendClient.inner_invoke("get_environment"); 
	}
	

	// From src-tauri/src/logic/exercises/commands.rs:27
	public static getExerciseDetails(category: number, id: number): Promise<ExerciseDetails> {
	  return BackendClient.inner_invoke("get_exercise_details", { category, id }); 
	}
	

	// From src-tauri/src/logic/exercises/commands.rs:16
	public static getExercises(): Promise<ExerciseListItem[]> {
	  return BackendClient.inner_invoke("get_exercises"); 
	}
	

	// From src-tauri/src/logic/exercises/commands.rs:40
	public static getExercisesCatalog(): Promise<Record<number, number[]>> {
	  return BackendClient.inner_invoke("get_exercises_catalog"); 
	}
	

	// From src-tauri/src/logic/sessions/commands.rs:82
	public static getHeatmapData(): Promise<[number, number][][]> {
	  return BackendClient.inner_invoke("get_heatmap_data"); 
	}
	

	// From src-tauri/src/logic/exercises/commands.rs:91
	public static getProgressionStatus(category: number, id: number): Promise<ProgressionSuggestion | null> {
	  return BackendClient.inner_invoke("get_progression_status", { category, id }); 
	}
	

	// From src-tauri/src/logic/exercises/commands.rs:102
	public static getProgressionSuggestion(category: number, id: number): Promise<ProgressionSuggestion | null> {
	  return BackendClient.inner_invoke("get_progression_suggestion", { category, id }); 
	}
	

	// From src-tauri/src/logic/devices/commands.rs:34
	public static getRegisteredDevices(): Promise<DeviceListItem[]> {
	  return BackendClient.inner_invoke("get_registered_devices"); 
	}
	

	// From src-tauri/src/logic/report/commands.rs:28
	public static getReportData(from: number, to: number): Promise<ReportData> {
	  return BackendClient.inner_invoke("get_report_data", { from, to }); 
	}
	

	// From src-tauri/src/logic/sessions/commands.rs:28
	public static getSessionDetails(timestamp: number): Promise<SessionDetails> {
	  return BackendClient.inner_invoke("get_session_details", { timestamp }); 
	}
	

	// From src-tauri/src/logic/sessions/commands.rs:15
	public static getSessions(limit: number | null): Promise<SessionListItem[]> {
	  return BackendClient.inner_invoke("get_sessions", { limit }); 
	}
	

	// From src-tauri/src/logic/app/commands.rs:14
	public static getSettings(): Promise<Settings> {
	  return BackendClient.inner_invoke("get_settings"); 
	}
	

	// From src-tauri/src/logic/app/commands.rs:49
	public static getTranslations(): Promise<Record<string, string>> {
	  return BackendClient.inner_invoke("get_translations"); 
	}
	

	// From src-tauri/src/logic/workouts/commands.rs:25
	public static getWorkoutDetails(name: string): Promise<WorkoutDetails> {
	  return BackendClient.inner_invoke("get_workout_details", { name }); 
	}
	

	// From src-tauri/src/logic/workouts/commands.rs:14
	public static getWorkoutList(): Promise<WorkoutListItem[]> {
	  return BackendClient.inner_invoke("get_workout_list"); 
	}
	

	// From src-tauri/src/logic/connect/commands.rs:12
	public static importFromConnect(): Promise<number> {
	  return BackendClient.inner_invoke("import_from_connect"); 
	}
	

	// From src-tauri/src/logic/devices/commands.rs:27
	public static importFromDevice(serial: string): Promise<number> {
	  return BackendClient.inner_invoke("import_from_device", { serial }); 
	}
	

	// From src-tauri/src/logic/sessions/commands.rs:71
	public static importFromFiles(): Promise<number> {
	  return BackendClient.inner_invoke("import_from_files"); 
	}
	

	// From src-tauri/src/logic/connect/commands.rs:25
	public static importWeightFromConnect(): Promise<number> {
	  return BackendClient.inner_invoke("import_weight_from_connect"); 
	}
	

	// From src-tauri/src/logic/report/commands.rs:10
	public static loadReport(from: number, to: number): Promise<void> {
	  return BackendClient.inner_invoke("load_report", { from, to }); 
	}
	

	// From src-tauri/src/logic/workouts/commands.rs:61
	public static loadWorkoutPage(workout: string): Promise<void> {
	  return BackendClient.inner_invoke("load_workout_page", { workout }); 
	}
	

	// From src-tauri/src/logic/app/commands.rs:24
	public static notifyFrontendReady(): Promise<void> {
	  return BackendClient.inner_invoke("notify_frontend_ready"); 
	}
	

	// From src-tauri/src/logic/report/commands.rs:19
	public static reportReady(): Promise<void> {
	  return BackendClient.inner_invoke("report_ready"); 
	}
	

	// From src-tauri/src/logic/exercises/commands.rs:138
	public static saveExerciseNotes(category: number, id: number, notes: string): Promise<void> {
	  return BackendClient.inner_invoke("save_exercise_notes", { category, id, notes }); 
	}
	

	// From src-tauri/src/logic/sessions/commands.rs:56
	public static saveSessionChanges(details: SessionStepsUpdate): Promise<void> {
	  return BackendClient.inner_invoke("save_session_changes", { details }); 
	}
	

	// From src-tauri/src/logic/workouts/commands.rs:47
	public static saveWorkout(edit: boolean, workout: Workout): Promise<void> {
	  return BackendClient.inner_invoke("save_workout", { edit, workout }); 
	}
	

	// From src-tauri/src/logic/connect/commands.rs:66
	public static sendToConnect(workout: string, serial: string): Promise<void> {
	  return BackendClient.inner_invoke("send_to_connect", { workout, serial }); 
	}
	

	// From src-tauri/src/logic/devices/commands.rs:12
	public static sendToDevice(workout: string, serial: string): Promise<void> {
	  return BackendClient.inner_invoke("send_to_device", { workout, serial }); 
	}
	

	// From src-tauri/src/logic/exercises/commands.rs:62
	public static setExerciseProgression(category: number, id: number, value: boolean): Promise<void> {
	  return BackendClient.inner_invoke("set_exercise_progression", { category, id, value }); 
	}
	

	// From src-tauri/src/logic/exercises/commands.rs:77
	public static setExerciseProgressionParameters(category: number, id: number, reps: string, weight: number): Promise<void> {
	  return BackendClient.inner_invoke("set_exercise_progression_parameters", { category, id, reps, weight }); 
	}
	

	// From src-tauri/src/logic/exercises/commands.rs:48
	public static setExerciseUnilateral(category: number, id: number, value: boolean): Promise<void> {
	  return BackendClient.inner_invoke("set_exercise_unilateral", { category, id, value }); 
	}
	

	// From src-tauri/src/logic/workouts/commands.rs:37
	public static setWorkoutStatus(workout: string, status: boolean): Promise<void> {
	  return BackendClient.inner_invoke("set_workout_status", { workout, status }); 
	}
	

	// From src-tauri/src/logic/app/commands.rs:42
	public static updateSettingsValue(name: string, value: string): Promise<void> {
	  return BackendClient.inner_invoke("update_settings_value", { name, value }); 
	}
	

	// From src-tauri/src/logic/export/commands.rs:11
	public static uploadToCloud(provider: CloudProvider): Promise<void> {
	  return BackendClient.inner_invoke("upload_to_cloud", { provider }); 
	}
	

	// From src-tauri/src/logic/workouts/commands.rs:70
	public static workoutPrintReady(workout: string): Promise<void> {
	  return BackendClient.inner_invoke("workout_print_ready", { workout }); 
	}
	

  
	private static inner_invoke<R>(method: string, payload?: InvokeArgs): Promise<R> {
		return new Promise<R>((resolve,reject)=>{
			const do_log = !BackendClient.DONT_LOG_COMMANDS.includes(method);
			if(do_log) {
				console.debug("Invoking command '"+method+"', payload: ", payload);
			}
			invoke<R>(method, payload).then((response)=>{
				if(do_log) {
					console.debug("Finished command '"+method+"', response: ", response);
				}
				resolve(response);
			}).catch((err) =>{
				if(do_log) {
					console.debug("Failed command '"+method+"', reason: ", err);
				}
				reject(err);
			});
		});
	}
}