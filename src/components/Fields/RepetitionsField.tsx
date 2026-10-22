import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { SxProps, TextField, Theme } from "@mui/material";
import React, { JSX, useContext, useState } from "react";

type Props = {
    label: string;
    value: number | undefined | null;
    min?: number;
    max?: number | undefined;
    // Clearing the field reports null instead of reverting
    allowEmpty?: boolean;
    error?: boolean;
    sx?: SxProps<Theme>;
    style?: React.CSSProperties;
    onChange: (reps: number | null) => void;
};

export function RepetitionsField({
    label,
    value,
    min = 0,
    max = undefined,
    allowEmpty = false,
    error,
    sx,
    style,
    onChange,
}: Props): JSX.Element {
    const { translate } = useContext(I18nSettingsContext);
    // Raw text while editing, so the parent value does not overwrite partial input
    const [input, setInput] = useState<string | null>(null);

    // Invalid input is reverted on the spot. The DOM value is reset by hand because a
    // number input reports "" for things like "-", which React would not overwrite
    const onInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        if (!e.target.validity.badInput && e.target.value == "" && allowEmpty) {
            setInput("");
            onChange(null);
            return;
        }

        const reps = Number(e.target.value);
        if (
            e.target.validity.badInput ||
            e.target.value == "" ||
            !Number.isInteger(reps) ||
            reps < min
        ) {
            setInput(null);
            e.target.value = value?.toString() ?? "";
            return;
        }

        setInput(e.target.value);
        onChange(reps);
    };

    return (
        <TextField
            label={translate(label)}
            error={error}
            type="number"
            size="small"
            sx={sx}
            style={style}
            value={input ?? value ?? ""}
            slotProps={{
                htmlInput: {
                    className: "no-spinner",
                    inputMode: "numeric",
                    min,
                    max: max !== undefined ? max : 999,
                    step: 1,
                },
            }}
            onChange={onInputChange}
            onBlur={() => setInput(null)}
        />
    );
}
