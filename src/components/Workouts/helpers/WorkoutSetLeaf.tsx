import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { StepType } from "@/utils/backend/models";
import { LeafStep, WorkoutUtils } from "@/utils/WorkoutUtils";
import {
    Box,
    FormControl,
    IconButton,
    InputLabel,
    MenuItem,
    Select,
    SelectChangeEvent,
    TextField,
    Tooltip,
} from "@mui/material";
import { useContext, useId, useState } from "react";
import ArrowUpwardIcon from "@mui/icons-material/ArrowUpward";
import ArrowDownwardIcon from "@mui/icons-material/ArrowDownward";
import ClearIcon from "@mui/icons-material/Clear";
import { ExerciseOption } from "@/context/AppContext";
import { ExerciseField } from "@/components/Fields/ExerciseField";
import { RepetitionsField } from "@/components/Fields/RepetitionsField";
import { WeightField } from "@/components/Fields/WeightField";

type Props = {
    leaf: LeafStep;
    index: number;
    sameLevel: number;
    exercisesCatalog: ExerciseOption[];
    onDelete: () => void;
    swapPosition: (pos1: number, pos2: number) => void;
    onChange: (leaf: LeafStep) => void;
};

enum LimitType {
    Time,
    Lap,
    Reps,
}

export function WorkoutSetLeaf({
    leaf,
    index,
    sameLevel,
    exercisesCatalog,
    onDelete,
    swapPosition,
    onChange,
}: Props) {
    const { translate } = useContext(I18nSettingsContext);
    const typeLabelId = useId();
    const limitLabelId = useId();
    const [timeInput, setTimeInput] = useState<string | null>(null);
    const isExercise = leaf.kind == StepType.Exercise;
    const errors = WorkoutUtils.leafErrors(leaf);
    const limit = isExercise
        ? leaf.reps != null
            ? LimitType.Reps
            : leaf.time != null
              ? LimitType.Time
              : LimitType.Lap
        : leaf.time != null
          ? LimitType.Time
          : LimitType.Lap;
    const formattedTime =
        leaf.time != null
            ? `${String(Math.floor(leaf.time / 60)).padStart(2, "0")}:${String(leaf.time % 60).padStart(2, "0")}`
            : "";
    const selected =
        exercisesCatalog.find(
            (e) => e.ex_cat == leaf.ex_cat && e.ex_id == leaf.ex_id,
        ) ?? undefined;

    const onTypeChange = (e: SelectChangeEvent<string>) => {
        switch (e.target.value as StepType) {
            case StepType.Exercise:
                onChange(WorkoutUtils.DEFAULT_EXERCISE);
                break;
            case StepType.Rest:
                onChange(WorkoutUtils.DEFAULT_REST);
                break;
        }
    };

    const onLimitChange = (e: SelectChangeEvent<number>) => {
        const obj = { ...leaf };
        switch (e.target.value as LimitType) {
            case LimitType.Lap:
                obj.reps = null;
                obj.time = null;
                break;
            case LimitType.Reps:
                obj.reps = 1;
                obj.time = null;
                break;
            case LimitType.Time:
                obj.time = 60;
                obj.reps = null;
                break;
        }
        onChange(obj);
    };

    const onWeightChange = (weight: number | null) => {
        onChange({
            ...leaf,
            weight: weight == null ? null : Math.round(weight * 10) / 10,
        });
    };

    const onRepsChange = (reps: number | null) => {
        onChange({
            ...leaf,
            reps,
        });
    };

    // Accepts "MM:SS", "M:S" or bare minutes. Keystrokes that break the pattern
    // (including seconds over 59) are dropped
    const onTimeChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        const value = e.target.value;
        if (!/^\d{0,3}(:([0-5]\d?)?)?$/.test(value)) {
            return;
        }
        setTimeInput(value);
        const [minutes, seconds = 0] = value
            .split(":")
            .map((p) => parseInt(p) || 0);
        onChange({
            ...leaf,
            time: minutes * 60 + seconds,
        });
    };

    const onExerciseChange = (opt: ExerciseOption) => {
        onChange({
            ...leaf,
            ex_cat: opt?.ex_cat ?? null,
            ex_id: opt?.ex_id ?? null,
        });
    };

    const typeSelect = (
        <FormControl size="small" sx={{ width: 120 }}>
            <InputLabel id={typeLabelId}>{translate("step_type")}</InputLabel>
            <Select
                labelId={typeLabelId}
                value={leaf.kind}
                label={translate("step_type")}
                onChange={onTypeChange}
                MenuProps={{
                    slotProps: { paper: { style: { maxHeight: 300 } } },
                }}
            >
                <MenuItem value={StepType.Exercise}>
                    {translate("step_type_exercise")}
                </MenuItem>
                <MenuItem value={StepType.Rest}>
                    {translate("step_type_rest")}
                </MenuItem>
            </Select>
        </FormControl>
    );

    const limitControls = (
        <>
            <FormControl size="small" sx={{ width: 140, marginRight: "20px" }}>
                <InputLabel id={limitLabelId}>
                    {translate("step_limit_type")}
                </InputLabel>
                <Select
                    labelId={limitLabelId}
                    value={limit}
                    label={translate("step_limit_type")}
                    onChange={onLimitChange}
                    MenuProps={{
                        slotProps: { paper: { style: { maxHeight: 300 } } },
                    }}
                >
                    <MenuItem value={LimitType.Time}>
                        {translate("limit_type_time")}
                    </MenuItem>
                    <MenuItem value={LimitType.Lap}>
                        {translate("limit_type_lap")}
                    </MenuItem>
                    {isExercise && (
                        <MenuItem value={LimitType.Reps}>
                            {translate("limit_type_reps")}
                        </MenuItem>
                    )}
                </Select>
            </FormControl>

            {isExercise && limit == LimitType.Reps && (
                <RepetitionsField
                    label="repetitions"
                    value={leaf.reps}
                    min={1}
                    error={errors.reps}
                    onChange={onRepsChange}
                    sx={{ width: 100 }}
                />
            )}
            {limit == LimitType.Time && (
                <TextField
                    label={translate("time") + " (MM:SS)"}
                    error={errors.time}
                    size="small"
                    sx={{ width: 120 }}
                    value={timeInput ?? formattedTime}
                    placeholder="00:00"
                    slotProps={{
                        htmlInput: {
                            inputMode: "numeric",
                        },
                    }}
                    onChange={onTimeChange}
                    onBlur={() => setTimeInput(null)}
                />
            )}

            {isExercise && (
                <WeightField
                    allowEmpty
                    error={errors.weight}
                    value={leaf.weight}
                    onChange={onWeightChange}
                    sx={{ width: 100 }}
                />
            )}
        </>
    );

    const notesBlk = (
        <Box
            sx={{
                py: 1,
            }}
        >
            <TextField
                label={translate("notes")}
                type="text"
                maxRows={1}
                size="small"
                sx={{ width: 500 }}
                value={leaf.notes ?? ""}
                slotProps={{
                    htmlInput: {
                        maxLength: 200,
                    },
                }}
                onChange={(e) => onChange({ ...leaf, notes: e.target.value })}
            />
        </Box>
    );

    const actions = (
        <Box
            sx={{
                display: "flex",
                gap: 0.5,
                flexDirection: "row",
                justifyContent: "flex-end",
                marginLeft: "auto",
            }}
        >
            <Tooltip title={translate("move_up")}>
                <span>
                    <IconButton
                        size="small"
                        color="inherit"
                        disabled={index == 0}
                        onClick={() => swapPosition(index, index - 1)}
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
                        onClick={() => swapPosition(index, index + 1)}
                    >
                        <ArrowDownwardIcon fontSize="small" />
                    </IconButton>
                </span>
            </Tooltip>
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
    );

    if (!isExercise) {
        return (
            <Box
                sx={{
                    display: "flex",
                    flexWrap: "wrap",
                    alignItems: "center",
                    gap: 0.5,
                    px: 2,
                    py: 2,
                }}
            >
                <Box sx={{ marginRight: "20px" }}>{typeSelect}</Box>
                {limitControls}
                {notesBlk}
                {actions}
            </Box>
        );
    }

    return (
        <>
            <Box
                sx={{
                    display: "flex",
                    flexWrap: "wrap",
                    alignItems: "center",
                    gap: 1,
                    px: 2,
                    py: 2,
                }}
            >
                {typeSelect}
                <ExerciseField
                    selected={selected}
                    onChange={(opt) => onExerciseChange(opt!)}
                    sx={{ width: 480 }}
                />
            </Box>

            <Box
                sx={{
                    display: "flex",
                    flexWrap: "wrap",
                    alignItems: "center",
                    gap: 0.5,
                    px: 2,
                    pb: 2,
                }}
            >
                {limitControls}
                {notesBlk}
                {actions}
            </Box>
        </>
    );
}
