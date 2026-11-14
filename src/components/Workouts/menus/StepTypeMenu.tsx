import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { StepType } from "@/utils/backend/models";
import { Divider, ListItemText, Menu, MenuItem } from "@mui/material";
import { useContext } from "react";

const STEP_TYPES = [
    StepType.Exercise,
    StepType.Warmup,
    StepType.Cooldown,
    StepType.Recovery,
    StepType.Rest,
    StepType.Other,
];

type Props = {
    anchor: HTMLElement | null;
    onClose: () => void;
    onSelect: (kind: StepType) => void;
    // When given, also offers a nested set (repeat group)
    onSelectGroup?: () => void;
};

/// Menu to pick the kind of a new step, anchored to the button that opened it.
export function StepTypeMenu({
    anchor,
    onClose,
    onSelect,
    onSelectGroup,
}: Props) {
    const { translate } = useContext(I18nSettingsContext);

    return (
        <Menu
            anchorEl={anchor}
            open={Boolean(anchor)}
            onClose={onClose}
            anchorOrigin={{ vertical: "bottom", horizontal: "right" }}
            transformOrigin={{ vertical: "top", horizontal: "right" }}
        >
            {STEP_TYPES.map((kind) => (
                <MenuItem
                    key={kind}
                    onClick={() => {
                        onClose();
                        onSelect(kind);
                    }}
                >
                    <ListItemText>
                        {translate("step_type_" + kind.toLowerCase())}
                    </ListItemText>
                </MenuItem>
            ))}
            {onSelectGroup && <Divider />}
            {onSelectGroup && (
                <MenuItem
                    onClick={() => {
                        onClose();
                        onSelectGroup();
                    }}
                >
                    <ListItemText>{translate("set")}</ListItemText>
                </MenuItem>
            )}
        </Menu>
    );
}
