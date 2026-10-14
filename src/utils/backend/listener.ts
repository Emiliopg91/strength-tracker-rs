//Auto generated file, do not edit manually

import { listen } from "@tauri-apps/api/event";

import { DeviceListItem, SessionLocation } from "./models";

export class BackendListener {
  public static onAddedWorkoutSteps(
    callback: (payload: string[]) => void,
  ): () => void {
    return BackendListener.inner_listen<string[]>(
      "added_workout_steps",
      callback,
    );
  }

  public static onDeviceConnected(
    callback: (payload: DeviceListItem) => void,
  ): () => void {
    return BackendListener.inner_listen<DeviceListItem>(
      "device_connected",
      callback,
    );
  }

  public static onDeviceDisconnected(
    callback: (payload: DeviceListItem) => void,
  ): () => void {
    return BackendListener.inner_listen<DeviceListItem>(
      "device_disconnected",
      callback,
    );
  }

  public static onFinishLoading(callback: () => void): () => void {
    return BackendListener.inner_listen<undefined>("finish_loading", callback);
  }

  public static onSessionLocationUpdate(
    callback: (payload: SessionLocation) => void,
  ): () => void {
    return BackendListener.inner_listen<SessionLocation>(
      "session_location_update",
      callback,
    );
  }

  public static onSessionsAdded(callback: () => void): () => void {
    return BackendListener.inner_listen<undefined>("sessions_added", callback);
  }

  public static onStartLoading(callback: () => void): () => void {
    return BackendListener.inner_listen<undefined>("start_loading", callback);
  }

  private static inner_listen<R>(
    event_name: string,
    callback: (payload: R) => void,
  ): () => void {
    console.debug("Listening to event '" + event_name + "'");
    const unlisten = listen<R>(event_name, (event) => {
      console.debug(
        "Received event for '" + event_name + "', payload: ",
        event.payload,
      );
      callback(event.payload);
    }).catch((err) => {
      console.error("Failed to listen to '" + event_name + "':", err);
      return () => {
        //Do nothing
      };
    });

    return () => {
      console.debug("Stopping listening to event '" + event_name + "'");
      unlisten.then((fn) => fn());
    };
  }
}
