import { AppContext, ExerciseOption } from "@/context/AppContext";
import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { Autocomplete, SxProps, TextField, Theme } from "@mui/material";
import { useContext } from "react";

type Props = {
    selected: ExerciseOption | undefined;
    onChange: (opt: ExerciseOption) => void;
    // Makes the field clearable
    onClear?: () => void;
    error?: boolean;
    sx?: SxProps<Theme>;
};

export function ExerciseField({
    selected,
    onChange,
    onClear,
    error,
    sx,
}: Props) {
    const { exercisesCatalog } = useContext(AppContext);
    const { translate } = useContext(I18nSettingsContext);

    return (
        <Autocomplete
            size="small"
            sx={sx}
            options={exercisesCatalog}
            // null instead of undefined keeps it controlled, otherwise MUI
            // ignores later values when the row gets reused for another step
            value={selected ?? (null as unknown as ExerciseOption)}
            disableClearable={!onClear}
            getOptionKey={(opt) => `${opt.ex_cat}-${opt.ex_id}`}
            isOptionEqualToValue={(a, b) =>
                a.ex_cat == b.ex_cat && a.ex_id == b.ex_id
            }
            onChange={(_, opt) => (opt ? onChange(opt) : onClear?.())}
            renderInput={(params) => (
                <TextField
                    {...params}
                    multiline
                    error={error}
                    label={translate("select_exercise")}
                />
            )}
        />
    );
}
