import { BackendClient } from "@/utils/backend/client";
import {
    DistanceUnit,
    Languages,
    Settings,
    WeightUnit,
} from "@/utils/backend/models";
import { useCallback, useEffect, useMemo, useState } from "react";
import { JSX } from "react/jsx-runtime";
import {
    DEFAULT_SETTINGS,
    I18nSettingsContext,
    KG_TO_LB,
    KM_TO_MI,
} from "./I18nSettingsContext";
import { LocalizationProvider } from "@mui/x-date-pickers/LocalizationProvider";
import { AdapterDateFns } from "@mui/x-date-pickers/AdapterDateFns";
import { enUS, es, type Locale } from "date-fns/locale";
import { format, parse } from "date-fns";

// Single source of locale for pickers, dates and numbers (Intl uses locale.code).
// Both locales start the week on Monday
const LOCALES: Record<Languages, Locale> = {
    [Languages.Spanish]: es,
    [Languages.English]: enUS,
};

// "P" is the locale's short date: dd/MM/yyyy (es) or MM/dd/yyyy (en-US)
const DATE_FORMAT = "P";
const TIME_DATE_FORMAT = "HH:mm P";

export function I18nSettingsProvider({
    children,
}: {
    children: JSX.Element;
}): JSX.Element {
    const [i18nReady, setI18nReady] = useState(false);
    const [settings, setSettings] = useState<Settings>(DEFAULT_SETTINGS);
    const [translations, setTranslations] = useState<Record<string, string>>(
        {},
    );

    const updateSettings = useCallback((changes: Partial<Settings>) => {
        setSettings((previous) => ({ ...previous, ...changes }));
    }, []);

    const toKm = useCallback(
        (value: number) => {
            switch (settings.distance_unit) {
                case DistanceUnit.Miles:
                    return value / KM_TO_MI;
                default:
                    return value;
            }
        },
        [settings.distance_unit],
    );

    const fromKm = useCallback(
        (km: number) => {
            switch (settings.distance_unit) {
                case DistanceUnit.Miles:
                    return km * KM_TO_MI;
                default:
                    return km;
            }
        },
        [settings.distance_unit],
    );

    const toKg = useCallback(
        (value: number) => {
            switch (settings.weight_unit) {
                case WeightUnit.Pounds:
                    return value / KG_TO_LB;
                default:
                    return value;
            }
        },
        [settings.weight_unit],
    );

    const fromKg = useCallback(
        (kg: number) => {
            switch (settings.weight_unit) {
                case WeightUnit.Pounds:
                    return kg * KG_TO_LB;
                default:
                    return kg;
            }
        },
        [settings.weight_unit],
    );

    const getDistanceUnit = useCallback(() => {
        switch (settings.distance_unit) {
            case DistanceUnit.Kilometers:
                return "Km";
            case DistanceUnit.Miles:
                return "Mi";
        }
    }, [settings.distance_unit]);

    const getWeightUnit = useCallback(() => {
        switch (settings.weight_unit) {
            case WeightUnit.Kilograms:
                return "Kg";
            case WeightUnit.Pounds:
                return "Lb";
        }
    }, [settings.weight_unit]);

    const locale = LOCALES[settings.language];

    // Intl.NumberFormat is costly to build, so instances are cached per locale and options
    const numberFormatters = useMemo(
        () => new Map<string, Intl.NumberFormat>(),
        // Recreated on locale change to drop formatters of the previous locale
        [locale],
    );

    const getNumberFormatter = useCallback(
        (options: Intl.NumberFormatOptions) => {
            const key = JSON.stringify(options);
            let formatter = numberFormatters.get(key);
            if (!formatter) {
                formatter = new Intl.NumberFormat(locale.code, {
                    useGrouping: "always",
                    ...options,
                });
                numberFormatters.set(key, formatter);
            }
            return formatter;
        },
        [locale, numberFormatters],
    );

    const formatNumber = useCallback(
        (value: number, decimals: number) =>
            getNumberFormatter({
                minimumFractionDigits: decimals,
                maximumFractionDigits: decimals,
            }).format(value),
        [getNumberFormatter],
    );

    const formatPercent = useCallback(
        (ratio: number, decimals: number, signed = false) =>
            getNumberFormatter({
                style: "percent",
                minimumFractionDigits: decimals,
                maximumFractionDigits: decimals,
                signDisplay: signed ? "always" : "auto",
            }).format(ratio),
        [getNumberFormatter],
    );

    const formatDuration = useCallback((seconds: number) => {
        if (seconds == 0) {
            return "0:00";
        }

        const h = Math.floor(seconds / 3600);
        const m = Math.floor((seconds % 3600) / 60);
        const s = Math.floor(seconds % 60);

        let res: string;
        if (h > 0) {
            res = `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
        } else {
            res = `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
        }

        while (res.startsWith("0") && !res.startsWith("0:")) {
            res = res.slice(1);
        }

        return res;
    }, []);

    const formatDate = useCallback(
        (date: number) => format(date * 1000, DATE_FORMAT, { locale }),
        [locale],
    );

    const formatTimeDate = useCallback(
        (date: number) => format(date * 1000, TIME_DATE_FORMAT, { locale }),
        [locale],
    );

    const parseLocalDateTime = useCallback(
        (dateStr: string) => {
            const local = parse(dateStr, TIME_DATE_FORMAT, new Date(), {
                locale,
            });

            // date-fns is lenient (e.g. "9:30"), so require an exact round-trip
            if (
                isNaN(local.getTime()) ||
                format(local, TIME_DATE_FORMAT, { locale }) !== dateStr
            ) {
                throw new Error("Wrong date format");
            }

            return local;
        },
        [locale],
    );

    const translate = useCallback(
        (key: string, replacements?: string[]) => {
            if (!translations[key]) {
                console.warn("Missing translation", key);
                return key;
            }

            let translation = translations[key];

            if (replacements) {
                replacements.forEach((r) => {
                    translation = translation.replace("{}", r);
                });
            }

            return translation;
        },
        [translations],
    );

    const refreshTranslations = useCallback(() => {
        BackendClient.getTranslations().then((translations) => {
            setTranslations(Object.freeze(translations));
        });
    }, []);

    useEffect(() => {
        BackendClient.getSettings()
            .then((settings) => {
                setSettings(settings);
            })
            .finally(() => {
                BackendClient.getTranslations()
                    .then((translations) => {
                        setTranslations(translations);
                    })
                    .finally(() => {
                        setI18nReady(true);
                    });
            });
    }, []);

    const value = useMemo(
        () => ({
            i18nReady,
            settings,
            updateSettings,
            translate,
            refreshTranslations,
            toKg,
            fromKg,
            toKm,
            fromKm,
            getDistanceUnit,
            getWeightUnit,
            formatDuration,
            formatDate,
            formatTimeDate,
            parseLocalDateTime,
            formatNumber,
            formatPercent,
        }),
        [
            i18nReady,
            settings,
            updateSettings,
            translate,
            refreshTranslations,
            toKg,
            fromKg,
            toKm,
            fromKm,
            getDistanceUnit,
            getWeightUnit,
            formatDuration,
            formatDate,
            formatTimeDate,
            parseLocalDateTime,
            formatNumber,
            formatPercent,
        ],
    );

    return (
        <I18nSettingsContext.Provider value={value}>
            <LocalizationProvider
                dateAdapter={AdapterDateFns}
                adapterLocale={locale}
            >
                {children}
            </LocalizationProvider>
        </I18nSettingsContext.Provider>
    );
}
