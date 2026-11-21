import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { StepType } from "@/utils/backend/models";
import { LeafStep, WorkoutUtils } from "@/utils/WorkoutUtils";
import {
    Box,
    FormControl,
    IconButton,
    InputAdornment,
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

enum LoadType {
    Weight,
    OneRmPercent,
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
    const { translate, formatNumber, getWeightUnit, fromKg } =
        useContext(I18nSettingsContext);
    const typeLabelId = useId();
    const limitLabelId = useId();
    const loadLabelId = useId();
    const loadType =
        leaf.one_rm_pct != null ? LoadType.OneRmPercent : LoadType.Weight;
    const [timeInput, setTimeInput] = useState<string | null>(null);
    const isActive = leaf.kind != StepType.Rest;
    const errors = WorkoutUtils.leafErrors(leaf);
    const limit = isActive
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
    // Live preview of the load a percentage resolves to; until the 1RMs load (one_rm undefined),
    // the load it resolved to when last saved
    const percentPreview: string | null = (() => {
        if (leaf.one_rm_pct == null || errors.oneRmPct) {
            return null;
        }
        if (selected?.one_rm === undefined) {
            return leaf.weight != null
                ? "≈ " + formatNumber(leaf.weight, 1) + " " + getWeightUnit()
                : null;
        }
        if (selected.one_rm === null) {
            return translate("no_recent_one_rm");
        }
        const kg = WorkoutUtils.percentLoad(
            selected.one_rm,
            leaf.one_rm_pct,
            selected.increment ?? 0,
        );
        return "≈ " + formatNumber(fromKg(kg), 1) + " " + getWeightUnit();
    })();
    const notesPlaceholder =
        leaf.kind == StepType.Exercise
            ? (selected?.notes ?? undefined)
            : undefined;

    const onTypeChange = (e: SelectChangeEvent<string>) => {
        const kind = e.target.value as StepType;
        const next = WorkoutUtils.getDefaultLeaf(kind);
        // Rests have no exercise; any other kind keeps the one already picked
        onChange(
            kind == StepType.Rest
                ? next
                : { ...next, ex_cat: leaf.ex_cat, ex_id: leaf.ex_id },
        );
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

    const onLoadTypeChange = (e: SelectChangeEvent<number>) => {
        onChange(
            (e.target.value as LoadType) == LoadType.OneRmPercent
                ? { ...leaf, weight: null, one_rm_pct: 70 }
                : { ...leaf, weight: 0, one_rm_pct: null },
        );
    };

    // Integers only; clearing the field leaves it invalid until a value is typed
    const onPercentChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        const value = e.target.value;
        if (!/^\d{0,3}$/.test(value)) {
            return;
        }
        onChange({
            ...leaf,
            one_rm_pct: value == "" ? 0 : parseInt(value),
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
            // A percentage resolved for the previous exercise no longer applies
            weight: leaf.one_rm_pct != null ? null : leaf.weight,
        });
    };

    const typeSelect = (
        <FormControl size="small" sx={{ width: 160 }}>
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
                <MenuItem value={StepType.Warmup}>
                    {translate("step_type_warmup")}
                </MenuItem>
                <MenuItem value={StepType.Cooldown}>
                    {translate("step_type_cooldown")}
                </MenuItem>
                <MenuItem value={StepType.Recovery}>
                    {translate("step_type_recovery")}
                </MenuItem>
                <MenuItem value={StepType.Rest}>
                    {translate("step_type_rest")}
                </MenuItem>
                <MenuItem value={StepType.Other}>
                    {translate("step_type_other")}
                </MenuItem>
            </Select>
        </FormControl>
    );

    const limitControls = (
        <>
            <FormControl size="small" sx={{ width: 140 }}>
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
                    {isActive && (
                        <MenuItem value={LimitType.Reps}>
                            {translate("limit_type_reps")}
                        </MenuItem>
                    )}
                </Select>
            </FormControl>

            {isActive && limit == LimitType.Reps && (
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

            {isActive && (
                <FormControl size="small" sx={{ width: 110 }}>
                    <InputLabel id={loadLabelId}>
                        {translate("load_type")}
                    </InputLabel>
                    <Select
                        labelId={loadLabelId}
                        value={loadType}
                        label={translate("load_type")}
                        onChange={onLoadTypeChange}
                    >
                        <MenuItem value={LoadType.Weight}>
                            {getWeightUnit()}
                        </MenuItem>
                        <MenuItem
                            value={LoadType.OneRmPercent}
                            // Percentages resolve against the 1RM of the exercise
                            disabled={leaf.ex_cat == null || leaf.ex_id == null}
                        >
                            {"% " + translate("rm")}
                        </MenuItem>
                    </Select>
                </FormControl>
            )}
            {isActive && loadType == LoadType.Weight && (
                <WeightField
                    allowEmpty
                    error={errors.weight}
                    value={leaf.weight}
                    onChange={onWeightChange}
                    sx={{ width: 100 }}
                />
            )}
            {isActive && loadType == LoadType.OneRmPercent && (
                <Tooltip title={translate("one_rm_pct_hint")}>
                    <TextField
                        label={"% " + translate("rm")}
                        error={errors.oneRmPct}
                        size="small"
                        sx={{ width: 170 }}
                        value={leaf.one_rm_pct || ""}
                        slotProps={{
                            htmlInput: {
                                inputMode: "numeric",
                                style: { textAlign: "center" },
                            },
                            input: {
                                endAdornment:
                                    percentPreview != null ? (
                                        <InputAdornment
                                            position="end"
                                            sx={{ ml: 0 }}
                                        >
                                            {percentPreview}
                                        </InputAdornment>
                                    ) : undefined,
                            },
                        }}
                        onChange={onPercentChange}
                    />
                </Tooltip>
            )}
        </>
    );

    const notesBlk = (
        <TextField
            label={translate("notes")}
            type="text"
            maxRows={1}
            size="small"
            sx={{ flex: 1, minWidth: 200 }}
            value={leaf.notes ?? ""}
            placeholder={notesPlaceholder}
            slotProps={{
                htmlInput: {
                    maxLength: 200,
                },
                // Keep the label shrunk so the exercise notes placeholder is visible
                inputLabel: notesPlaceholder ? { shrink: true } : undefined,
            }}
            onChange={(e) => onChange({ ...leaf, notes: e.target.value })}
        />
    );

    const actions = (
        <Box
            sx={{
                display: "flex",
                gap: 0.5,
                flexDirection: "row",
                justifyContent: "flex-end",
                alignItems: "center",
                // Same height as a single-line field, so the buttons line up with its first line
                height: 40,
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

    return (
        <Box
            sx={{
                display: "flex",
                flexDirection: "column",
                gap: 1,
                px: 1,
                py: 1.5,
            }}
        >
            <Box
                sx={{
                    display: "flex",
                    flexWrap: "wrap",
                    // A multiline exercise grows downwards without dragging the rest of the row
                    alignItems: "flex-start",
                    gap: 1,
                }}
            >
                {typeSelect}
                {isActive && (
                    <ExerciseField
                        selected={selected}
                        singleLine
                        error={errors.exercise}
                        onChange={(opt) => onExerciseChange(opt!)}
                        onClear={
                            leaf.kind != StepType.Exercise
                                ? () =>
                                      onChange({
                                          ...leaf,
                                          ex_cat: null,
                                          ex_id: null,
                                          // Without an exercise there is no 1RM
                                          ...(leaf.one_rm_pct != null && {
                                              weight: 0,
                                              one_rm_pct: null,
                                          }),
                                      })
                                : undefined
                        }
                        sx={{ flex: 1, minWidth: 250 }}
                    />
                )}
                {actions}
            </Box>

            <Box
                sx={{
                    display: "flex",
                    flexWrap: "wrap",
                    alignItems: "center",
                    gap: 1,
                }}
            >
                {limitControls}
                {notesBlk}
            </Box>
        </Box>
    );
}
