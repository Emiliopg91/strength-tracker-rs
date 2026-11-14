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

type Props = {
    day: Date;
    workouts: string[];
    onAdd: (workout: string) => void;
    onClose: () => void;
};

export function PlanWorkoutModal({ day, workouts, onAdd, onClose }: Props) {
    const { translate, formatDate } = useContext(I18nSettingsContext);
    const [workout, setWorkout] = useState(workouts[0] ?? "");

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
                        onChange={(e) => setWorkout(e.target.value)}
                    >
                        {workouts.map((name) => (
                            <MenuItem key={name} value={name}>
                                {name}
                            </MenuItem>
                        ))}
                    </Select>
                </FormControl>
                <hr />
                <Button
                    variant="contained"
                    className="full-width-button"
                    disabled={workout == ""}
                    onClick={() => onAdd(workout)}
                >
                    {translate("add")}
                </Button>
            </DialogContent>
        </Dialog>
    );
}
