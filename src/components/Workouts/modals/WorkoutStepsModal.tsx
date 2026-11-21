import { StepType, Workout, WorkoutDetails } from "@/utils/backend/models";
import {
    Button,
    Dialog,
    DialogContent,
    DialogTitle,
    IconButton,
    TextField,
} from "@mui/material";
import CloseIcon from "@mui/icons-material/Close";
import { useContext, useState } from "react";
import { WorkoutStepData, WorkoutUtils } from "@/utils/WorkoutUtils";
import { WorkoutSetGroup } from "../steps/WorkoutSetGroup";
import { WorkoutSetLeaf } from "../steps/WorkoutSetLeaf";
import { StepTypeMenu } from "../menus/StepTypeMenu";
import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { BackendClient } from "@/utils/backend/client";
import { useLoadingTask } from "@/hooks/useLoadingTask";
import { AppContext } from "@/context/AppContext";

type Props = {
    workout: WorkoutDetails;
    isEdit: boolean;
    onClose: () => void;
};

export function WorkoutStepsModal({ workout, isEdit, onClose }: Props) {
    const withLoading = useLoadingTask();
    const { translate, fromKg, toKg } = useContext(I18nSettingsContext);
    const { exercisesCatalog: fullCatalog } = useContext(AppContext);
    // Exercise notes only reach the steps of workouts with progression, so they aren't offered as
    // the placeholder of the step notes otherwise
    const exercisesCatalog = workout.progression
        ? fullCatalog
        : fullCatalog.map((e) => ({ ...e, notes: null }));

    const [name, setName] = useState(workout?.name);
    const [notes, setNotes] = useState(workout?.notes);
    const [originalNotes] = useState(() => workout?.notes);
    const [originalSteps] = useState(() =>
        JSON.stringify(WorkoutUtils.parseWorkoutSteps(workout!.steps, fromKg)),
    );
    const [steps, setSteps] = useState(
        WorkoutUtils.parseWorkoutSteps(workout!.steps, fromKg),
    );
    const [addMenuAnchor, setAddMenuAnchor] = useState<HTMLElement | null>(
        null,
    );
    const valid = name.trim().length > 0 && WorkoutUtils.validateSteps(steps);
    const changed =
        name.trim() != workout.name ||
        (notes ?? "") != (originalNotes ?? "") ||
        JSON.stringify(steps) != originalSteps;
    const canSave = valid && (!isEdit || changed);

    const swapPosition = (pos1: number, pos2: number) => {
        const tmp = steps[pos1];
        const nSteps = [...steps];
        nSteps[pos1] = nSteps[pos2];
        nSteps[pos2] = tmp;
        setSteps(nSteps);
    };

    const updateStep = (pos: number, nStep: WorkoutStepData) => {
        setSteps(steps.map((step, idx) => (idx == pos ? nStep : step)));
    };

    const addGroup = () => {
        setSteps([...steps, structuredClone(WorkoutUtils.DEFAULT_GROUP)]);
    };

    const addStep = (kind: StepType) => {
        setSteps([
            ...steps,
            structuredClone(WorkoutUtils.getDefaultLeaf(kind)),
        ]);
    };

    const deleteStep = (i: number) => {
        const nSteps = steps.filter((_, idx) => idx != i);
        setSteps(nSteps);
    };

    const saveWorkout = () => {
        if (canSave) {
            // A new workout goes with id 0 and gets its own once stored
            const newWorkout: Workout = {
                id: isEdit ? workout.id : 0,
                enabled: workout.enabled,
                progression: workout.progression,
                name: name.trim(),
                steps: WorkoutUtils.toWorkoutSteps(
                    steps,
                    isEdit ? workout.id : 0,
                    toKg,
                ),
                notes,
                connect_id: null,
                connect_created_time: null,
            };
            withLoading(BackendClient.saveWorkout(isEdit, newWorkout)).then(
                () => {
                    onClose();
                },
            );
        }
    };

    const onNameChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        const name = e.target.value;
        setName(name);
    };

    return (
        <>
            {exercisesCatalog.length > 0 && (
                <Dialog open={true} onClose={onClose} fullWidth maxWidth="md">
                    <DialogTitle>
                        <TextField
                            placeholder={translate("workout")}
                            error={name.trim().length == 0}
                            onChange={onNameChange}
                            value={name}
                            sx={{ width: 600 }}
                        ></TextField>
                        <IconButton
                            onClick={onClose}
                            className="modal-close-button"
                        >
                            <CloseIcon />
                        </IconButton>
                    </DialogTitle>

                    <DialogContent dividers>
                        <TextField
                            label={translate("notes")}
                            type="text"
                            maxRows={1}
                            size="small"
                            sx={{ width: 685, marginBottom: "20px" }}
                            value={notes ?? ""}
                            slotProps={{
                                htmlInput: {
                                    maxLength: 200,
                                },
                            }}
                            onChange={(e) => setNotes(e.target.value)}
                        />
                        <br />
                        {steps.map((step, idx) =>
                            WorkoutUtils.isStepGroup(step) ? (
                                <WorkoutSetGroup
                                    group={step}
                                    index={idx}
                                    sameLevel={steps.length}
                                    exercisesCatalog={exercisesCatalog}
                                    key={`group-${idx}`}
                                    swapPosition={swapPosition}
                                    onChange={(g) => updateStep(idx, g)}
                                    onDelete={() => {
                                        deleteStep(idx);
                                    }}
                                />
                            ) : (
                                // Same frame as a group, without the repeat legend
                                <fieldset
                                    key={`leaf-${idx}`}
                                    style={{
                                        backgroundColor: "rgba(0, 0, 0, 0.15)",
                                        borderRadius: "10px",
                                        padding: "0 8px",
                                        marginBottom: "12px",
                                    }}
                                >
                                    <WorkoutSetLeaf
                                        leaf={step}
                                        index={idx}
                                        sameLevel={steps.length}
                                        exercisesCatalog={exercisesCatalog}
                                        swapPosition={swapPosition}
                                        onChange={(l) => updateStep(idx, l)}
                                        onDelete={() => deleteStep(idx)}
                                    />
                                </fieldset>
                            ),
                        )}
                        <div
                            style={{
                                display: "flex",
                                flexDirection: "row",
                                justifyContent: "flex-end",
                                alignItems: "center",
                                gap: "8px",
                            }}
                        >
                            <Button
                                variant="contained"
                                onClick={(e) =>
                                    setAddMenuAnchor(e.currentTarget)
                                }
                            >
                                {translate("add")}
                            </Button>
                            <StepTypeMenu
                                anchor={addMenuAnchor}
                                onClose={() => setAddMenuAnchor(null)}
                                onSelect={addStep}
                                onSelectGroup={addGroup}
                            />
                            <Button
                                color="success"
                                variant="contained"
                                onClick={saveWorkout}
                                disabled={!canSave}
                            >
                                {translate("save_workout")}
                            </Button>
                        </div>
                    </DialogContent>
                </Dialog>
            )}
        </>
    );
}
