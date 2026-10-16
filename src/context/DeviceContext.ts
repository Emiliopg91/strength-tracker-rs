import { DeviceListItem } from "@/utils/backend/models";
import { createContext } from "react";

interface DeviceContextType {
    registeredDevices: DeviceListItem[];
    availableDevices: DeviceListItem[];
}

const defaultValue: DeviceContextType = {
    registeredDevices: [],
    availableDevices: [],
};

export const DeviceContext = createContext(defaultValue);
