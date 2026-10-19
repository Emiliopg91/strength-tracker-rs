import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { TextField } from "@mui/material";
import { JSX, useContext, useState } from "react";

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
    const { translate } = useContext(I18nSettingsContext);

    const terms = normalize(filter).split(/\s+/).filter(Boolean);
    const filtered =
        terms.length === 0
            ? items
            : items.filter((e) => {
                  const texts = getTexts(e).map(normalize);
                  return terms.every((t) => texts.some((s) => s.includes(t)));
              });

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
            sx={{ mt: 1, mx: 1 }}
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
