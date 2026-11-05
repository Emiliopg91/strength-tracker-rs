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
    value: number | undefined | null;
    // Read-only target shown after the value ("80/100"); the value may exceed it
    reference?: number | null;
    // Clearing the field reports null instead of reverting
    allowEmpty?: boolean;
    error?: boolean;
    sx?: SxProps<Theme>;
    style?: React.CSSProperties;
    onChange: (weight: number | null) => void;
};

export function WeightField({
    value,
    reference,
    allowEmpty = false,
    error,
    sx,
    style,
    onChange,
}: Props): JSX.Element {
    const { translate, getWeightUnit } = useContext(I18nSettingsContext);
    // Raw text while editing, so a rounded parent value does not overwrite what is typed
    const [input, setInput] = useState<string | null>(null);

    // Invalid input is reverted on the spot. The DOM value is reset by hand because a
    // number input reports "" for things like "-", which React would not overwrite
    const onInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        if (!e.target.validity.badInput && e.target.value == "" && allowEmpty) {
            setInput("");
            onChange(null);
            return;
        }

        const weight = parseFloat(e.target.value);
        if (e.target.validity.badInput || isNaN(weight) || weight < 0) {
            setInput(null);
            e.target.value = value?.toString() ?? "";
            return;
        }

        setInput(e.target.value);
        onChange(weight);
    };

    const hasReference = reference != null;
    const displayed = input ?? value ?? "";

    return (
        <TextField
            label={translate("weight") + " (" + getWeightUnit() + ")"}
            error={error}
            type="number"
            size="small"
            sx={sx}
            style={style}
            value={displayed}
            slotProps={{
                htmlInput: {
                    className: "no-spinner",
                    inputMode: "decimal",
                    min: 0,
                    max: 999,
                    step: 0.1,
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
