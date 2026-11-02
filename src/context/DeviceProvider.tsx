import { BackendClient } from "@/utils/backend/client";
import { BackendListener } from "@/utils/backend/listener";
import { DeviceListItem } from "@/utils/backend/models";
import { useContext, useEffect, useEffectEvent, useState } from "react";
import { JSX } from "react/jsx-runtime";
import { AvailableDevice, DeviceContext } from "./DeviceContext";
import { useLoadingTask } from "@/hooks/useLoadingTask";
import { AppContext } from "./AppContext";

/// Updates `device` in `prev` with `patch`, adding it with `defaults` + `patch` if missing
function upsert(
    prev: AvailableDevice[],
    device: DeviceListItem,
    defaults: Omit<AvailableDevice, "manufacturer" | "model" | "serial_number">,
    patch: Partial<AvailableDevice>,
): AvailableDevice[] {
    if (prev.some((d) => d.serial_number == device.serial_number)) {
        return prev.map((p) =>
            p.serial_number == device.serial_number ? { ...p, ...patch } : p,
        );
    }
    return [
        ...prev,
        {
            manufacturer: device.manufacturer,
            model: device.model,
            serial_number: device.serial_number,
            ...defaults,
            ...patch,
        },
    ];
}

export function DeviceProvider({
    children,
}: {
    children: JSX.Element;
}): JSX.Element {
    const [devices, setDevices] = useState<AvailableDevice[]>([]);
    const withLoading = useLoadingTask();
    const { connectUser } = useContext(AppContext);

    const loadRegisteredDevices = useEffectEvent(() => {
        withLoading(BackendClient.getRegisteredDevices()).then((registered) => {
            // Entries already set by events are fresher than the database
            setDevices((prev) =>
                registered.reduce(
                    (acc, device) =>
                        acc.some((d) => d.serial_number == device.serial_number)
                            ? acc
                            : upsert(
                                  acc,
                                  device,
                                  // Not reachable until a connection or discovery event says so
                                  {
                                      usb: false,
                                      connect: false,
                                      strength: device.strength_workout,
                                  },
                                  {},
                              ),
                    prev,
                ),
            );
        });
    });

    useEffect(() => {
        const unregisterDiscovery = BackendListener.onDeviceDiscovered(
            (device) => {
                setDevices((prev) =>
                    upsert(
                        prev,
                        device,
                        { usb: false, connect: true, strength: false },
                        { connect: true, strength: device.strength_workout },
                    ),
                );
            },
        );
        const unregisterConnection = BackendListener.onDeviceConnected(
            (device) => {
                setDevices((prev) =>
                    upsert(
                        prev,
                        device,
                        { usb: true, connect: false, strength: true },
                        { usb: true, strength: true },
                    ),
                );
            },
        );

        // Unplugged devices stay listed: they are registered and can still get pending workouts
        const unregisterDisconnection = BackendListener.onDeviceDisconnected(
            (device) => {
                setDevices((prev) =>
                    prev.map((p) =>
                        p.serial_number == device.serial_number
                            ? { ...p, usb: false }
                            : p,
                    ),
                );
            },
        );

        loadRegisteredDevices();

        return () => {
            unregisterDiscovery();
            unregisterConnection();
            unregisterDisconnection();
        };
    }, []);

    const value = {
        // Logged out, no device is reachable through Connect; logging in rediscovers them
        devices: connectUser
            ? devices
            : devices.map((d) => ({ ...d, connect: false })),
    };

    return (
        <DeviceContext.Provider value={value}>
            {children}
        </DeviceContext.Provider>
    );
}
