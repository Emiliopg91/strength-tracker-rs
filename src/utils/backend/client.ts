//Auto generated file, do not edit manually

import { invoke, InvokeArgs } from "@tauri-apps/api/core";

import { AppEnvironment, BodyMetricListItem, CloudProvider, DeviceListItem, ExerciseDetails, ExerciseListItem, ProgressionSuggestion, ReportData, SessionDetails, SessionListItem, SessionStepsUpdate, Settings, Workout, WorkoutDetails, WorkoutListItem } from "./models";

export class BackendClient {

    private static DONT_LOG_COMMANDS: string[] = [];

	// From src-tauri/src/logic/mod.rs:537
	public static addBodyMeasures(measures: BodyMetricListItem): Promise<void> {
	  return BackendClient.inner_invoke("add_body_measures", { measures }); 
	}
	

	// From src-tauri/src/logic/mod.rs:364
	public static applyProgressionSuggestion(category: number, id: number, suggestion: ProgressionSuggestion): Promise<void> {
	  return BackendClient.inner_invoke("apply_progression_suggestion", { category, id, suggestion }); 
	}
	

	// From src-tauri/src/logic/mod.rs:358
	public static availableAnyProgressionSuggestions(): Promise<boolean> {
	  return BackendClient.inner_invoke("available_any_progression_suggestions"); 
	}
	

	// From src-tauri/src/logic/mod.rs:599
	public static connectLoggedUser(): Promise<string | null> {
	  return BackendClient.inner_invoke("connect_logged_user"); 
	}
	

	// From src-tauri/src/logic/mod.rs:607
	public static connectLogin(): Promise<string | null> {
	  return BackendClient.inner_invoke("connect_login"); 
	}
	

	// From src-tauri/src/logic/mod.rs:616
	public static connectLogout(): Promise<void> {
	  return BackendClient.inner_invoke("connect_logout"); 
	}
	

	// From src-tauri/src/logic/mod.rs:547
	public static deleteBodyMetric(date: number): Promise<void> {
	  return BackendClient.inner_invoke("delete_body_metric", { date }); 
	}
	

	// From src-tauri/src/logic/mod.rs:237
	public static downloadSession(timestamp: number): Promise<void> {
	  return BackendClient.inner_invoke("download_session", { timestamp }); 
	}
	

	// From src-tauri/src/logic/mod.rs:568
	public static exportDatabase(): Promise<void> {
	  return BackendClient.inner_invoke("export_database"); 
	}
	

	// From src-tauri/src/logic/mod.rs:528
	public static getBodyMeasures(): Promise<BodyMetricListItem[]> {
	  return BackendClient.inner_invoke("get_body_measures"); 
	}
	

	// From src-tauri/src/logic/mod.rs:481
	public static getEnvironment(): Promise<AppEnvironment> {
	  return BackendClient.inner_invoke("get_environment"); 
	}
	

	// From src-tauri/src/logic/mod.rs:283
	public static getExerciseDetails(category: number, id: number): Promise<ExerciseDetails> {
	  return BackendClient.inner_invoke("get_exercise_details", { category, id }); 
	}
	

	// From src-tauri/src/logic/mod.rs:274
	public static getExercises(): Promise<ExerciseListItem[]> {
	  return BackendClient.inner_invoke("get_exercises"); 
	}
	

	// From src-tauri/src/logic/mod.rs:294
	public static getExercisesCatalog(): Promise<Record<number, number[]>> {
	  return BackendClient.inner_invoke("get_exercises_catalog"); 
	}
	

	// From src-tauri/src/logic/mod.rs:266
	public static getHeatmapData(): Promise<[number, number][][]> {
	  return BackendClient.inner_invoke("get_heatmap_data"); 
	}
	

	// From src-tauri/src/logic/mod.rs:338
	public static getProgressionStatus(category: number, id: number): Promise<ProgressionSuggestion | null> {
	  return BackendClient.inner_invoke("get_progression_status", { category, id }); 
	}
	

	// From src-tauri/src/logic/mod.rs:348
	public static getProgressionSuggestion(category: number, id: number): Promise<ProgressionSuggestion | null> {
	  return BackendClient.inner_invoke("get_progression_suggestion", { category, id }); 
	}
	

	// From src-tauri/src/logic/mod.rs:461
	public static getRegisteredDevices(): Promise<DeviceListItem[]> {
	  return BackendClient.inner_invoke("get_registered_devices"); 
	}
	

	// From src-tauri/src/logic/mod.rs:515
	public static getReportData(from: number, to: number): Promise<ReportData> {
	  return BackendClient.inner_invoke("get_report_data", { from, to }); 
	}
	

	// From src-tauri/src/logic/mod.rs:227
	public static getSessionDetails(timestamp: number): Promise<SessionDetails> {
	  return BackendClient.inner_invoke("get_session_details", { timestamp }); 
	}
	

	// From src-tauri/src/logic/mod.rs:217
	public static getSessions(limit: number | null): Promise<SessionListItem[]> {
	  return BackendClient.inner_invoke("get_sessions", { limit }); 
	}
	

	// From src-tauri/src/logic/mod.rs:469
	public static getSettings(): Promise<Settings> {
	  return BackendClient.inner_invoke("get_settings"); 
	}
	

	// From src-tauri/src/logic/mod.rs:493
	public static getTranslations(): Promise<Record<string, string>> {
	  return BackendClient.inner_invoke("get_translations"); 
	}
	

	// From src-tauri/src/logic/mod.rs:402
	public static getWorkoutDetails(name: string): Promise<WorkoutDetails> {
	  return BackendClient.inner_invoke("get_workout_details", { name }); 
	}
	

	// From src-tauri/src/logic/mod.rs:393
	public static getWorkoutList(): Promise<WorkoutListItem[]> {
	  return BackendClient.inner_invoke("get_workout_list"); 
	}
	

	// From src-tauri/src/logic/mod.rs:579
	public static importFromConnect(): Promise<number> {
	  return BackendClient.inner_invoke("import_from_connect"); 
	}
	

	// From src-tauri/src/logic/mod.rs:455
	public static importFromDevice(serial: string): Promise<number> {
	  return BackendClient.inner_invoke("import_from_device", { serial }); 
	}
	

	// From src-tauri/src/logic/mod.rs:257
	public static importFromFiles(): Promise<number> {
	  return BackendClient.inner_invoke("import_from_files"); 
	}
	

	// From src-tauri/src/logic/mod.rs:589
	public static importWeightFromConnect(): Promise<number> {
	  return BackendClient.inner_invoke("import_weight_from_connect"); 
	}
	

	// From src-tauri/src/logic/mod.rs:503
	public static loadReport(from: number, to: number): Promise<void> {
	  return BackendClient.inner_invoke("load_report", { from, to }); 
	}
	

	// From src-tauri/src/logic/mod.rs:430
	public static loadWorkoutPage(workout: string): Promise<void> {
	  return BackendClient.inner_invoke("load_workout_page", { workout }); 
	}
	

	// From src-tauri/src/logic/mod.rs:475
	public static notifyFrontendReady(): Promise<void> {
	  return BackendClient.inner_invoke("notify_frontend_ready"); 
	}
	

	// From src-tauri/src/logic/mod.rs:509
	public static reportReady(): Promise<void> {
	  return BackendClient.inner_invoke("report_ready"); 
	}
	

	// From src-tauri/src/logic/mod.rs:378
	public static saveExerciseNotes(category: number, id: number, notes: string): Promise<void> {
	  return BackendClient.inner_invoke("save_exercise_notes", { category, id, notes }); 
	}
	

	// From src-tauri/src/logic/mod.rs:247
	public static saveSessionChanges(details: SessionStepsUpdate): Promise<void> {
	  return BackendClient.inner_invoke("save_session_changes", { details }); 
	}
	

	// From src-tauri/src/logic/mod.rs:418
	public static saveWorkout(edit: boolean, workout: Workout): Promise<void> {
	  return BackendClient.inner_invoke("save_workout", { edit, workout }); 
	}
	

	// From src-tauri/src/logic/mod.rs:622
	public static sendToConnect(workout: string, serial: string): Promise<void> {
	  return BackendClient.inner_invoke("send_to_connect", { workout, serial }); 
	}
	

	// From src-tauri/src/logic/mod.rs:444
	public static sendToDevice(workout: string, serial: string): Promise<void> {
	  return BackendClient.inner_invoke("send_to_device", { workout, serial }); 
	}
	

	// From src-tauri/src/logic/mod.rs:312
	public static setExerciseProgression(category: number, id: number, value: boolean): Promise<void> {
	  return BackendClient.inner_invoke("set_exercise_progression", { category, id, value }); 
	}
	

	// From src-tauri/src/logic/mod.rs:324
	public static setExerciseProgressionParameters(category: number, id: number, reps: string, weight: number): Promise<void> {
	  return BackendClient.inner_invoke("set_exercise_progression_parameters", { category, id, reps, weight }); 
	}
	

	// From src-tauri/src/logic/mod.rs:300
	public static setExerciseUnilateral(category: number, id: number, value: boolean): Promise<void> {
	  return BackendClient.inner_invoke("set_exercise_unilateral", { category, id, value }); 
	}
	

	// From src-tauri/src/logic/mod.rs:412
	public static setWorkoutStatus(workout: string, status: boolean): Promise<void> {
	  return BackendClient.inner_invoke("set_workout_status", { workout, status }); 
	}
	

	// From src-tauri/src/logic/mod.rs:487
	public static updateSettingsValue(name: string, value: string): Promise<void> {
	  return BackendClient.inner_invoke("update_settings_value", { name, value }); 
	}
	

	// From src-tauri/src/logic/mod.rs:559
	public static uploadToCloud(provider: CloudProvider): Promise<void> {
	  return BackendClient.inner_invoke("upload_to_cloud", { provider }); 
	}
	

	// From src-tauri/src/logic/mod.rs:436
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