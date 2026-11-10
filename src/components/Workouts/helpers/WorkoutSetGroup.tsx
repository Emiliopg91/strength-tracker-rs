import { StepGroup, WorkoutStepData, WorkoutUtils } from "@/utils/WorkoutUtils";
import { StepType } from "@/utils/backend/models";
import { WorkoutSetLeaf } from "./WorkoutSetLeaf";
import { StepTypeMenu } from "./StepTypeMenu";
import {
    Box,
    Divider,
    FormControl,
    IconButton,
    InputLabel,
    MenuItem,
    Select,
    SelectChangeEvent,
    Stack,
    Tooltip,
} from "@mui/material";
import { useContext, useId, useState } from "react";
import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import ArrowUpwardIcon from "@mui/icons-material/ArrowUpward";
import ArrowDownwardIcon from "@mui/icons-material/ArrowDownward";
import ClearIcon from "@mui/icons-material/Clear";
import AddCircleOutlineRoundedIcon from "@mui/icons-material/AddCircleOutlineRounded";
import { ExerciseOption } from "@/context/AppContext";

type Props = {
    group: StepGroup;
    index: number;
    sameLevel: number;
    exercisesCatalog: ExerciseOption[];
    onDelete: () => void;
    swapPosition: (pos1: number, pos2: number) => void;
    onChange: (group: StepGroup) => void;
};

export function WorkoutSetGroup({
    group,
    index,
    sameLevel,
    exercisesCatalog,
    onDelete,
    swapPosition,
    onChange,
}: Props) {
    const { translate } = useContext(I18nSettingsContext);
    const lapsLabelId = useId();
    const [addMenuAnchor, setAddMenuAnchor] = useState<HTMLElement | null>(
        null,
    );
    const children = group.wrapped;

    const onLapChanged = (e: SelectChangeEvent<number>) => {
        onChange({ ...group, laps: e.target.value as number });
    };

    const setChildren = (wrapped: WorkoutStepData[]) => {
        onChange({ ...group, wrapped });
    };

    const swapChildPosition = (pos1: number, pos2: number) => {
        const nChildren = [...children];
        nChildren[pos1] = children[pos2];
        nChildren[pos2] = children[pos1];
        setChildren(nChildren);
    };

    const updateChild = (i: number, child: WorkoutStepData) => {
        setChildren(children.map((c, idx) => (idx == i ? child : c)));
    };

    const deleteChild = (i: number) => {
        setChildren(children.filter((_, idx) => idx != i));
    };

    const addStep = (kind: StepType) => {
        const leaf = WorkoutUtils.getDefaultLeaf(kind);
        setChildren([...children, structuredClone(leaf)]);
    };

    const addGroup = () => {
        setChildren([...children, structuredClone(WorkoutUtils.DEFAULT_GROUP)]);
    };

    return (
        <>
            <fieldset
                style={{
                    backgroundColor: "rgba(0, 0, 0, 0.15)",
                    borderRadius: "10px",
                    padding: "0 8px 4px",
                    marginBottom: "12px",
                }}
            >
                <legend>
                    {/* The legend breaks the border along its whole width, so the actions sit on the same line */}
                    <Box
                        sx={{
                            display: "flex",
                            alignItems: "center",
                            gap: 1,
                            px: 0.5,
                        }}
                    >
                        <FormControl
                            size="small"
                            sx={{
                                minWidth: 100,
                                // Straddles the fieldset border, so it needs an opaque background.
                                // --Paper-overlay is inherited from the dialog's Paper (dark-mode elevation tint).
                                "& .MuiInputBase-root": {
                                    backgroundColor: "background.paper",
                                    backgroundImage: "var(--Paper-overlay)",
                                },
                            }}
                        >
                            <InputLabel id={lapsLabelId}>
                                {translate("repeat")}
                            </InputLabel>
                            <Select
                                labelId={lapsLabelId}
                                value={group.laps}
                                label={translate("repeat")}
                                onChange={onLapChanged}
                                MenuProps={{
                                    slotProps: {
                                        paper: { style: { maxHeight: 300 } },
                                    },
                                }}
                            >
                                {Array.from(
                                    { length: 50 },
                                    (_, i) => i + 1,
                                ).map((n) => (
                                    <MenuItem key={n} value={n}>
                                        {n}
                                    </MenuItem>
                                ))}
                            </Select>
                        </FormControl>
                        <Box sx={{ display: "flex", gap: 0.5 }}>
                            <Tooltip title={translate("move_up")}>
                                <span>
                                    <IconButton
                                        size="small"
                                        color="inherit"
                                        disabled={index == 0}
                                        onClick={() =>
                                            swapPosition(index, index - 1)
                                        }
                                    >
                                        <ArrowUpwardIcon fontSize="small" />
                                    </IconButton>
                                </span>
                            </Tooltip>
                            <Tooltip title={translate("move_down")}>
                                <span>
                                    <IconButton
                                        size="small"
                                        color="inherit"
                                        disabled={index == sameLevel - 1}
                                        onClick={() =>
                                            swapPosition(index, index + 1)
                                        }
                                    >
                                        <ArrowDownwardIcon fontSize="small" />
                                    </IconButton>
                                </span>
                            </Tooltip>
                            <Tooltip title={translate("add_step")}>
                                <IconButton
                                    size="small"
                                    onClick={(e) =>
                                        setAddMenuAnchor(e.currentTarget)
                                    }
                                >
                                    <AddCircleOutlineRoundedIcon fontSize="small" />
                                </IconButton>
                            </Tooltip>
                            <StepTypeMenu
                                anchor={addMenuAnchor}
                                onClose={() => setAddMenuAnchor(null)}
                                onSelect={addStep}
                                onSelectGroup={addGroup}
                            />
                            <Tooltip title={translate("delete")}>
                                <span>
                                    <IconButton
                                        size="small"
                                        color="error"
                                        disabled={sameLevel <= 1}
                                        onClick={onDelete}
                                    >
                                        <ClearIcon fontSize="small" />
                                    </IconButton>
                                </span>
                            </Tooltip>
                        </Box>
                    </Box>
                </legend>
                <Stack
                    divider={
                        <Divider
                            flexItem
                            sx={{
                                borderBottomWidth: 2,
                                borderColor: "rgba(255, 255, 255, 0.25)",
                            }}
                        />
                    }
                >
                    {children.map((child, idx) =>
                        WorkoutUtils.isStepGroup(child) ? (
                            // Nested repeat; the spacing keeps its legend clear of the dividers
                            <Box key={`group-${idx}`} sx={{ pt: 1 }}>
                                <WorkoutSetGroup
                                    group={child}
                                    index={idx}
                                    exercisesCatalog={exercisesCatalog}
                                    sameLevel={children.length}
                                    swapPosition={swapChildPosition}
                                    onChange={(g) => updateChild(idx, g)}
                                    onDelete={() => deleteChild(idx)}
                                />
                            </Box>
                        ) : (
                            <WorkoutSetLeaf
                                key={`leaf-${idx}`}
                                leaf={child}
                                index={idx}
                                exercisesCatalog={exercisesCatalog}
                                sameLevel={children.length}
                                swapPosition={swapChildPosition}
                                onChange={(l) => updateChild(idx, l)}
                                onDelete={() => deleteChild(idx)}
                            />
                        ),
                    )}
                </Stack>
            </fieldset>
        </>
    );
}
