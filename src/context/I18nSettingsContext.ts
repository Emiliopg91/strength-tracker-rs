import {
  DistanceUnit,
  Languages,
  Settings,
  WeightUnit,
} from "@/utils/backend/models";
import { createContext } from "react";

export const KG_TO_LB = 2.20462;
export const KM_TO_MI = 0.621371;

export const DEFAULT_SETTINGS: Settings = {
  distance_unit: DistanceUnit.Kilometers,
  weight_unit: WeightUnit.Kilograms,
  start_boot: false,
  language: Languages.English,
  on_device_connect: false,
  close_to_tray: false,
  start_into_tray: false,
};

interface I18nSettingsContextType {
  i18nReady: boolean;
  settings: Settings;
  updateSettings: (changes: Partial<Settings>) => void;
  translate: (key: string, replacements?: string[]) => string;
  refreshTranslations: () => void;
  fromKg: (kg: number) => number;
  toKg: (value: number) => number;
  fromKm: (km: number) => number;
  toKm: (value: number) => number;
  getDistanceUnit: () => string;
  getWeightUnit: () => string;
  formatDuration: (seconds: number) => string;
  formatDate: (date: number) => string;
  formatTimeDate: (date: number) => string;
  parseLocalDateTime: (dateStr: string) => Date;
  formatNumber: (value: number, decimals: number) => string;
  formatPercent: (ratio: number, decimals: number, signed?: boolean) => string;
}

const defaultValue: I18nSettingsContextType = {
  i18nReady: false,
  settings: DEFAULT_SETTINGS,
  updateSettings: () => {
    /* empty */
  },
  translate: () => {
    return "";
  },
  refreshTranslations: () => {
    /* empty */
  },
  fromKg: (kg: number) => {
    return kg;
  },
  toKg: (value: number) => {
    return value;
  },
  fromKm: (km: number) => {
    return km;
  },
  toKm: (value: number) => {
    return value;
  },
  getDistanceUnit: () => {
    return "";
  },
  getWeightUnit: () => {
    return "";
  },
  formatDuration: () => {
    return "";
  },
  formatDate: () => {
    return "";
  },
  formatTimeDate: () => {
    return "";
  },
  parseLocalDateTime: () => {
    return new Date();
  },
  formatNumber: (value: number, decimals: number) => {
    return value.toFixed(decimals);
  },
  formatPercent: (ratio: number, decimals: number) => {
    return `${(ratio * 100).toFixed(decimals)}%`;
  },
};

export const I18nSettingsContext = createContext(defaultValue);
