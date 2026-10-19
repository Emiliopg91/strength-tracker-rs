import { BackendClient } from "@/utils/backend/client";
import { BackendListener } from "@/utils/backend/listener";
import { DeviceListItem } from "@/utils/backend/models";
import { useEffect, useState } from "react";
import { JSX } from "react/jsx-runtime";
import { DeviceContext } from "./DeviceContext";

export function DeviceProvider({
    children,
}: {
    children: JSX.Element;
}): JSX.Element {
    const [availableDevices, setAvailableDevices] = useState<DeviceListItem[]>(
        [],
    );
    const [registeredDevices, setRegisteredDevices] = useState<
        DeviceListItem[]
    >([]);

    useEffect(() => {
        const unregisterConnection = BackendListener.onDeviceConnected(
            (device) => {
                setAvailableDevices((prev) => [...prev, device]);

                setRegisteredDevices((prev) =>
                    prev.some((d) => d.serial_number === device.serial_number)
                        ? prev
                        : [...prev, device],
                );
            },
        );

        const unregisterDisconnection = BackendListener.onDeviceDisconnected(
            (device) => {
                setAvailableDevices((prev) =>
                    prev.filter(
                        (d) => d.serial_number !== device.serial_number,
                    ),
                );
            },
        );

        BackendClient.getRegisteredDevices().then((devices) => {
            setRegisteredDevices(devices);
        });

        return () => {
            unregisterConnection();
            unregisterDisconnection();
        };
    }, []);

    const value = {
        registeredDevices,
        availableDevices,
    };

    return (
        <DeviceContext.Provider value={value}>
            {children}
        </DeviceContext.Provider>
    );
}
