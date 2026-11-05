import { SxProps, Theme } from "@mui/material";
import { CSSProperties } from "react";

// MUI positions outlined labels from the left edge. These match its small size offsets
export const centeredLabelSx: SxProps<Theme> = {
    left: "50%",
    width: "max-content",
    transformOrigin: "top center",
    transform: "translate(-50%, 9px) scale(1)",
    "&.MuiInputLabel-shrink": {
        transform: "translate(-50%, -9px) scale(0.75)",
    },
};

// Moves the gap in the outline under the centered label. Engines differ on whether a
// fieldset legend is aligned through its margins or its text-align, so both are set
export const centeredOutlineSx = {
    "& legend": {
        marginInline: "auto",
        textAlign: "center",
    },
} satisfies SxProps<Theme>;

// With a reference, the value and "/reference" are centered together as one block
export const referenceOutlineSx = {
    ...centeredOutlineSx,
    justifyContent: "center",
    px: "6px",
} satisfies SxProps<Theme>;

// MUI stretches the input over the free space, which would separate the value from the
// reference. The extra pixel leaves room for the caret after the last digit
export const referenceInputStyle = (text: string): CSSProperties => ({
    flex: "none",
    width: `calc(${Math.max(text.length, 1)}ch + 1px)`,
    paddingLeft: 0,
    textAlign: "right",
});
