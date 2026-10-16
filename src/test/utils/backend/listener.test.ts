import { beforeEach, describe, expect, it, Mock, vi } from "vitest";
import { EventCallback, listen, UnlistenFn } from "@tauri-apps/api/event";
import { BackendListener } from "@/utils/backend/listener";
import { DeviceListItem } from "@/utils/backend/models";

vi.mock("@tauri-apps/api/event", () => ({
    listen: vi.fn(),
}));

const listenMock = vi.mocked(listen);

const device: DeviceListItem = {
    manufacturer: "Garmin",
    model: "Forerunner",
    serial_number: "123",
};

describe("BackendListener", () => {
    let handlers: Map<string, EventCallback<unknown>>;
    let unlistenFn: Mock<UnlistenFn>;

    beforeEach(() => {
        handlers = new Map();
        unlistenFn = vi.fn<UnlistenFn>();
        listenMock.mockReset();
        listenMock.mockImplementation(
            async (event: string, handler: EventCallback<unknown>) => {
                handlers.set(event, handler);
                return unlistenFn;
            },
        );
        vi.spyOn(console, "debug").mockImplementation(() => undefined);
        vi.spyOn(console, "error").mockImplementation(() => undefined);
    });

    const emit = (event: string, payload: unknown) =>
        handlers.get(event)!({ event, id: 0, payload });

    it("subscribes to the backend event name and forwards the payload", () => {
        const callback = vi.fn();
        BackendListener.onDeviceConnected(callback);

        expect(listenMock).toHaveBeenCalledWith(
            "device_connected",
            expect.any(Function),
        );
        emit("device_connected", device);
        expect(callback).toHaveBeenCalledWith(device);
    });

    it("forwards the workout names of added_workout_steps", () => {
        const callback = vi.fn();
        BackendListener.onAddedWorkoutSteps(callback);

        expect(listenMock).toHaveBeenCalledWith(
            "added_workout_steps",
            expect.any(Function),
        );
        emit("added_workout_steps", ["Push", "Pull"]);
        expect(callback).toHaveBeenCalledWith(["Push", "Pull"]);
    });

    it("invokes payload-less callbacks", () => {
        const callback = vi.fn();
        BackendListener.onSessionsAdded(callback);

        emit("sessions_added", undefined);
        expect(callback).toHaveBeenCalledTimes(1);
    });

    it("unsubscribes once the listener is registered", async () => {
        const unlisten = BackendListener.onStartLoading(vi.fn());

        unlisten();
        await vi.waitFor(() => expect(unlistenFn).toHaveBeenCalledTimes(1));
    });

    it("returns a harmless unsubscribe when registration fails", async () => {
        listenMock.mockRejectedValueOnce(new Error("no IPC"));

        const unlisten = BackendListener.onFinishLoading(vi.fn());

        expect(() => unlisten()).not.toThrow();
        await vi.waitFor(() =>
            expect(console.error).toHaveBeenCalledWith(
                "Failed to listen to 'finish_loading':",
                expect.any(Error),
            ),
        );
        expect(unlistenFn).not.toHaveBeenCalled();
    });
});
