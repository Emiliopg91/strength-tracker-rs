import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { BackendClient } from "@/utils/backend/client";
import { CloudProvider } from "@/utils/backend/models";

vi.mock("@tauri-apps/api/core", () => ({
    invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

describe("BackendClient", () => {
    beforeEach(() => {
        invokeMock.mockReset();
        vi.spyOn(console, "debug").mockImplementation(() => undefined);
    });

    it("invokes commands without payload", async () => {
        invokeMock.mockResolvedValue([]);

        await expect(BackendClient.getExercises()).resolves.toEqual([]);
        expect(invokeMock).toHaveBeenCalledWith("get_exercises", undefined);
    });

    it("passes arguments using the backend parameter names", async () => {
        invokeMock.mockResolvedValue(undefined);

        await BackendClient.getSessions(5);
        await BackendClient.getExerciseDetails(1, 2);
        await BackendClient.setWorkoutStatus("Push", false);
        await BackendClient.uploadToCloud(CloudProvider.DropBox);

        expect(invokeMock.mock.calls).toEqual([
            ["get_sessions", { limit: 5 }],
            ["get_exercise_details", { category: 1, id: 2 }],
            ["set_workout_status", { workout: "Push", status: false }],
            ["upload_to_cloud", { provider: CloudProvider.DropBox }],
        ]);
    });

    it("resolves with the backend response", async () => {
        invokeMock.mockResolvedValue(3);

        await expect(BackendClient.importFromDevice("ABC")).resolves.toBe(3);
    });

    it("propagates backend errors", async () => {
        invokeMock.mockRejectedValue("Device not found");

        await expect(BackendClient.importFromDevice("ABC")).rejects.toBe(
            "Device not found",
        );
    });
});
