import { Workout, WorkoutDetails } from "@/utils/backend/models";
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
import { WorkoutSetGroup } from "./helpers/WorkoutSetGroup";
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
    const { exercisesCatalog } = useContext(AppContext);

    const [name, setName] = useState(workout?.name);
    const [notes, setNotes] = useState(workout?.notes);
    const [originalNotes] = useState(() => workout?.notes);
    const [originalSteps] = useState(() =>
        JSON.stringify(WorkoutUtils.parseWorkoutSteps(workout!.steps, fromKg)),
    );
    const [steps, setSteps] = useState(
        WorkoutUtils.parseWorkoutSteps(workout!.steps, fromKg),
    );
    const valid = name.length > 0 && WorkoutUtils.validateSteps(steps);
    const changed =
        name != workout.name ||
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

    const deleteGroup = (i: number) => {
        const nSteps = steps.filter((_, idx) => idx != i);
        setSteps(nSteps);
    };

    const saveWorkout = () => {
        if (canSave) {
            const newWorkout: Workout = {
                enabled: workout.enabled,
                name,
                steps: WorkoutUtils.toWorkoutSteps(steps, name, toKg),
                notes,
                connect_id: null,
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
                        {isEdit && <span>{name}</span>}
                        {!isEdit && (
                            <TextField
                                placeholder={translate("workout")}
                                error={name.length == 0}
                                onChange={onNameChange}
                                value={name}
                                sx={{ width: 600 }}
                            ></TextField>
                        )}
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
                                        deleteGroup(idx);
                                    }}
                                />
                            ) : (
                                <>
                                    1x
                                    <br />
                                    {step.kind}
                                    <hr />
                                </>
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
                            <Button variant="contained" onClick={addGroup}>
                                {translate("add_set")}
                            </Button>
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
