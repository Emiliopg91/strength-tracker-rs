import { useContext, useState } from "react";
import {
    Button,
    Dialog,
    DialogContent,
    DialogTitle,
    FormControl,
    IconButton,
    InputLabel,
    MenuItem,
    Select,
} from "@mui/material";
import CloseIcon from "@mui/icons-material/Close";
import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { WorkoutListItem } from "@/utils/backend/models";

type Props = {
    day: Date;
    workouts: WorkoutListItem[];
    onAdd: (workout: number) => void;
    onClose: () => void;
};

export function PlanWorkoutModal({ day, workouts, onAdd, onClose }: Props) {
    const { translate, formatDate } = useContext(I18nSettingsContext);
    // 0 when there is none: ids start at 1
    const [workout, setWorkout] = useState(workouts[0]?.id ?? 0);

    return (
        <Dialog open={true} onClose={onClose} fullWidth maxWidth="xs">
            <DialogTitle>
                {translate("plan_workout")}
                <IconButton onClick={onClose} className="modal-close-button">
                    <CloseIcon />
                </IconButton>
            </DialogTitle>

            <DialogContent dividers>
                <div className="plan-modal-date">
                    {formatDate(Math.floor(day.getTime() / 1000))}
                </div>
                <FormControl fullWidth size="small">
                    <InputLabel id="plan-workout-label">
                        {translate("workout")}
                    </InputLabel>
                    <Select
                        labelId="plan-workout-label"
                        label={translate("workout")}
                        value={workout}
                        onChange={(e) => setWorkout(Number(e.target.value))}
                    >
                        {workouts.map((w) => (
                            <MenuItem key={w.id} value={w.id}>
                                {w.name}
                            </MenuItem>
                        ))}
                    </Select>
                </FormControl>
                <hr />
                <Button
                    variant="contained"
                    className="full-width-button"
                    disabled={workout == 0}
                    onClick={() => onAdd(workout)}
                >
                    {translate("add")}
                </Button>
            </DialogContent>
        </Dialog>
    );
}
