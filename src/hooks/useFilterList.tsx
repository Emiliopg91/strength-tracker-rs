import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { TextField } from "@mui/material";
import { JSX, useContext, useMemo, useRef, useState } from "react";

function normalize(s: string): string {
    return s
        .normalize("NFD")
        .replace(/\p{Diacritic}/gu, "")
        .toLowerCase();
}

export function useFilterList<T>(
    items: T[],
    getTexts: (item: T) => string[],
): [JSX.Element, T[]] {
    const [filter, setFilter] = useState("");
    const i18n = useContext(I18nSettingsContext);
    const { translate } = i18n;

    // Callers pass inline closures, so keep the latest one in a ref
    // instead of making it a memo dependency. Those closures usually rely on
    // i18n (translate, date formatters), so depend on the context value to
    // refilter when the language or locale changes
    const getTextsRef = useRef(getTexts);
    getTextsRef.current = getTexts;

    const filtered = useMemo(() => {
        const terms = normalize(filter).split(/\s+/).filter(Boolean);
        if (terms.length === 0) {
            return items;
        }

        return items.filter((e) => {
            const texts = getTextsRef.current(e).map(normalize);
            return terms.every((t) => texts.some((s) => s.includes(t)));
        });
    }, [items, filter, i18n]);

    const input = (
        <TextField
            value={filter}
            helperText={
                filtered.length === 0
                    ? undefined
                    : filtered.length === 1
                      ? translate("1_entry")
                      : translate("x_entries", [String(filtered.length)])
            }
            placeholder={translate("filter")}
            size="small"
            onChange={(e) => setFilter(e.target.value)}
            sx={{ mt: 1 }}
            slotProps={{
                formHelperText: {
                    sx: {
                        fontSize: "0.85rem",
                        margin: "auto",
                    },
                },
            }}
            variant="standard"
        />
    );

    return [input, filtered];
}
