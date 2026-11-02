import { createContext } from "react";

export interface AvailableDevice {
    manufacturer: string;
    model: string;
    serial_number: string;
    usb: boolean;
    connect: boolean;
    strength: boolean;
}

interface DeviceContextType {
    devices: AvailableDevice[];
}

const defaultValue: DeviceContextType = {
    devices: [],
};

export const DeviceContext = createContext(defaultValue);
