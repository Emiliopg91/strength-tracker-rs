import { LeafStep, StepGroup, WorkoutUtils } from "@/utils/WorkoutUtils";
import { StepType } from "@/utils/backend/models";
import { WorkoutSetLeaf } from "./WorkoutSetLeaf";
import {
    Box,
    Divider,
    FormControl,
    IconButton,
    InputLabel,
    ListItemText,
    Menu,
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
    const leafs = group.wrapped;

    const onLapChanged = (e: SelectChangeEvent<number>) => {
        onChange({ ...group, laps: e.target.value as number });
    };

    const setLeafs = (wrapped: LeafStep[]) => {
        onChange({ ...group, wrapped });
    };

    const swapLeafPosition = (pos1: number, pos2: number) => {
        const nLeafs = [...leafs];
        nLeafs[pos1] = leafs[pos2];
        nLeafs[pos2] = leafs[pos1];
        setLeafs(nLeafs);
    };

    const updateLeaf = (i: number, leaf: LeafStep) => {
        setLeafs(leafs.map((l, idx) => (idx == i ? leaf : l)));
    };

    const deleteLeaf = (i: number) => {
        setLeafs(leafs.filter((_, idx) => idx != i));
    };

    const addStep = (kind: StepType) => {
        setAddMenuAnchor(null);
        const leaf =
            kind == StepType.Rest
                ? WorkoutUtils.DEFAULT_REST
                : WorkoutUtils.DEFAULT_EXERCISE;
        setLeafs([...leafs, structuredClone(leaf)]);
    };

    return (
        <>
            <fieldset
                style={{
                    backgroundColor: "rgba(0, 0, 0, 0.15)",
                    borderRadius: "10px",
                }}
            >
                <legend>
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
                            {Array.from({ length: 50 }, (_, i) => i + 1).map(
                                (n) => (
                                    <MenuItem key={n} value={n}>
                                        {n}
                                    </MenuItem>
                                ),
                            )}
                        </Select>
                    </FormControl>
                </legend>
                <Box
                    sx={{
                        display: "flex",
                        flexWrap: "wrap",
                        justifyContent: "space-between",
                        alignItems: "center",
                        gap: 1,
                        pb: 1,
                    }}
                >
                    <Box sx={{ display: "flex", gap: 0.5 }}>
                        <Tooltip title={translate("move_up")}>
                            <span>
                                <IconButton
                                    color="inherit"
                                    disabled={index == 0}
                                    onClick={() =>
                                        swapPosition(index, index - 1)
                                    }
                                >
                                    <ArrowUpwardIcon />
                                </IconButton>
                            </span>
                        </Tooltip>
                        <Tooltip title={translate("move_down")}>
                            <span>
                                <IconButton
                                    color="inherit"
                                    disabled={index == sameLevel - 1}
                                    onClick={() =>
                                        swapPosition(index, index + 1)
                                    }
                                >
                                    <ArrowDownwardIcon />
                                </IconButton>
                            </span>
                        </Tooltip>
                        <Tooltip title={translate("add_step")}>
                            <IconButton
                                onClick={(e) =>
                                    setAddMenuAnchor(e.currentTarget)
                                }
                            >
                                <AddCircleOutlineRoundedIcon />
                            </IconButton>
                        </Tooltip>
                        <Menu
                            anchorEl={addMenuAnchor}
                            open={Boolean(addMenuAnchor)}
                            onClose={() => setAddMenuAnchor(null)}
                            anchorOrigin={{
                                vertical: "bottom",
                                horizontal: "right",
                            }}
                            transformOrigin={{
                                vertical: "top",
                                horizontal: "right",
                            }}
                        >
                            <MenuItem
                                onClick={() => addStep(StepType.Exercise)}
                            >
                                <ListItemText>
                                    {translate("step_type_exercise")}
                                </ListItemText>
                            </MenuItem>
                            <MenuItem onClick={() => addStep(StepType.Rest)}>
                                <ListItemText>
                                    {translate("step_type_rest")}
                                </ListItemText>
                            </MenuItem>
                        </Menu>
                        <Tooltip title={translate("delete")}>
                            <span>
                                <IconButton
                                    color="error"
                                    disabled={sameLevel <= 1}
                                    onClick={onDelete}
                                >
                                    <ClearIcon />
                                </IconButton>
                            </span>
                        </Tooltip>
                    </Box>
                </Box>
                <Divider />
                <Stack divider={<Divider flexItem />} sx={{ px: 1 }}>
                    {leafs.map((leaf, idx) => (
                        <WorkoutSetLeaf
                            key={`leaf-${idx}`}
                            leaf={leaf}
                            index={idx}
                            exercisesCatalog={exercisesCatalog}
                            sameLevel={leafs.length}
                            swapPosition={swapLeafPosition}
                            onChange={(l) => updateLeaf(idx, l)}
                            onDelete={() => deleteLeaf(idx)}
                        />
                    ))}
                </Stack>
                <Divider />
            </fieldset>
            <br />
        </>
    );
}
