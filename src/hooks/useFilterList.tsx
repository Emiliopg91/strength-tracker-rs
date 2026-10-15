import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { TextField } from "@mui/material";
import { JSX, useContext, useMemo, useState } from "react";

export function useFilterList<T>(
  items: T[],
  getTexts: (item: T) => string[],
): [JSX.Element, T[]] {
  const [filter, setFilter] = useState("");
  const { translate } = useContext(I18nSettingsContext);

  const filtered = useMemo(() => {
    const term = filter.trim().toLowerCase();
    if (term.length === 0) {
      return items;
    }

    return items.filter((e) => {
      return (
        getTexts(e).find((s) => s.toLowerCase().includes(term)) !== undefined
      );
    });
  }, [items, filter, getTexts]);

  const input = (
    <TextField
      value={filter}
      placeholder={translate("filter")}
      size="small"
      onChange={(e) => setFilter(e.target.value)}
    />
  );

  return [input, filtered];
}
