import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { InputAdornment, SxProps, TextField, Theme } from "@mui/material";
import React, { JSX, useContext, useState } from "react";
import {
    centeredLabelSx,
    centeredOutlineSx,
    referenceInputStyle,
    referenceOutlineSx,
} from "./centeredFieldStyles";

type Props = {
    label: string;
    value: number | undefined | null;
    min?: number;
    max?: number | undefined;
    // Read-only target shown after the value ("12/15"); the value may exceed it
    reference?: number | null;
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
    reference,
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

    const hasReference = reference != null;
    const displayed = input ?? value ?? "";

    return (
        <TextField
            label={translate(label)}
            error={error}
            type="number"
            size="small"
            sx={sx}
            style={style}
            value={displayed}
            slotProps={{
                htmlInput: {
                    className: "no-spinner",
                    inputMode: "numeric",
                    min,
                    max: max !== undefined ? max : 999,
                    step: 1,
                    style: hasReference
                        ? referenceInputStyle(String(displayed))
                        : { textAlign: "center" },
                },
                input: {
                    sx: hasReference ? referenceOutlineSx : centeredOutlineSx,
                    endAdornment: hasReference ? (
                        <InputAdornment position="end" sx={{ ml: 0 }}>
                            {`/${reference}`}
                        </InputAdornment>
                    ) : undefined,
                },
                inputLabel: {
                    sx: centeredLabelSx,
                    // An empty field would otherwise draw the label over the reference
                    shrink: hasReference ? true : undefined,
                },
            }}
            onChange={onInputChange}
            onBlur={() => setInput(null)}
        />
    );
}
