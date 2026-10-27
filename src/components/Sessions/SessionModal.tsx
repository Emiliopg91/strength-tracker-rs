import { useLoadingTask } from "@/hooks/useLoadingTask";
import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { BackendClient } from "@/utils/backend/client";
import { SessionSetsUpdate } from "@/utils/backend/models";
import { useContext, useState } from "react";
import {
    Button,
    Dialog,
    DialogContent,
    DialogTitle,
    IconButton,
    TextareaAutosize,
} from "@mui/material";
import CloseIcon from "@mui/icons-material/Close";
import { SessionFrontDetails } from "@/utils/SessionUtils";
import { SessionMap } from "./helpers/SessionMap";
import { SessionDetailsTable } from "./helpers/SessionDetailsTable";
import { SessionHeartRateChart } from "./helpers/SessionHeartRateChart";
import { SessionSetsTable } from "./helpers/SessionSetsTable";
import "@/styles/Sessions/SessionModal.css";
import { MuscleModel } from "../Exercises/helpers/MuscleModel";

type Props = {
    session: SessionFrontDetails;
    onClose: () => void;
    onUpdate: () => void;
};

export function SessionModal({ session, onClose, onUpdate }: Props) {
    const withLoading = useLoadingTask();
    const { translate, toKg } = useContext(I18nSettingsContext);
    const [originalSession] = useState(session);
    const [localSession, setLocalSession] = useState({ ...session });
    const [notes, setNotes] = useState(session.notes);

    const changed =
        notes != originalSession.notes ||
        localSession.sets.some(
            (serie, idx) =>
                serie.reps != originalSession.sets[idx].reps ||
                serie.weight != originalSession.sets[idx].weight ||
                serie.ex_cat != originalSession.sets[idx].ex_cat ||
                serie.ex_id != originalSession.sets[idx].ex_id,
        );

    const updateSerieExercise = (
        idx: number,
        newCat: number,
        newId: number,
    ) => {
        setLocalSession((prev) => {
            const sets = prev.sets.map((s, i) =>
                i == idx ? { ...s, ex_cat: newCat, ex_id: newId } : s,
            );

            return { ...prev, sets };
        });
    };

    const updateSerieRepsWeight = (
        idx: number,
        field: "reps" | "weight",
        value: number,
    ) => {
        setLocalSession((prev) => {
            const sets = prev.sets.slice();
            sets[idx] = { ...sets[idx], [field]: value };

            return {
                ...prev,
                sets,
            };
        });
    };

    const updateNote = (notes: string) => {
        setNotes(notes);
        setLocalSession((prev) => ({ ...prev, notes }));
    };

    const saveChanges = () => {
        const update: SessionSetsUpdate = {
            timestamp: localSession.timestamp,
            sets: [],
            notes: notes.length > 0 ? notes : null,
        };
        localSession.sets.forEach((serie, serIdx) => {
            if (
                originalSession.sets[serIdx].reps != serie.reps ||
                originalSession.sets[serIdx].weight != serie.weight ||
                originalSession.sets[serIdx].ex_cat != serie.ex_cat ||
                originalSession.sets[serIdx].ex_id != serie.ex_id
            ) {
                update.sets.push({
                    ...serie,
                    weight: toKg(serie.weight),
                });
            }
        });
        withLoading(BackendClient.saveSessionChanges(update)).then(() => {
            onUpdate();
            onClose();
        });
    };

    return (
        <Dialog open={true} onClose={onClose} fullWidth maxWidth="sm">
            <DialogTitle>
                {translate("sport_" + localSession.sport) +
                    " - " +
                    translate(
                        "sport_" +
                            localSession.sport +
                            "_" +
                            localSession.sub_sport,
                    )}
                {localSession.name.length > 0 && (
                    <span>
                        :{" "}
                        <span className="session-modal-name">
                            {localSession.name}
                        </span>
                    </span>
                )}
                <IconButton onClick={onClose} className="modal-close-button">
                    <CloseIcon />
                </IconButton>
            </DialogTitle>

            <DialogContent dividers>
                {localSession.valid_points.length > 0 && (
                    <>
                        <SessionMap
                            timestamp={localSession.timestamp}
                            validPoints={localSession.valid_points}
                            gpsSegments={localSession.gps_segments}
                            startPoint={localSession.start_point}
                            finishPoint={localSession.finish_point}
                            laps={localSession.laps}
                        />
                        <hr />
                    </>
                )}
                <div className="session-details-row">
                    <SessionDetailsTable session={localSession} />
                    {(localSession.primary_muscles.length > 0 ||
                        localSession.secondary_muscles.length > 0) && (
                        <div className="session-muscle-model">
                            <MuscleModel
                                primary={localSession.primary_muscles}
                                secondary={localSession.secondary_muscles}
                            />
                        </div>
                    )}
                </div>
                {localSession.hrBreathData.length > 0 && (
                    <>
                        <hr />
                        <SessionHeartRateChart
                            hrBreathData={localSession.hrBreathData}
                            hrRanges={localSession.hrRanges}
                            zonesTimes={localSession.zones_times}
                            totalElapsedTime={localSession.total_elapsed_time}
                        />
                        <br />
                    </>
                )}
                {localSession.sets &&
                    Object.keys(localSession.sets).length > 0 && (
                        <SessionSetsTable
                            series={localSession.sets}
                            onUpdateSerieRepsWeight={updateSerieRepsWeight}
                            onUpdateSerieExercise={updateSerieExercise}
                        />
                    )}
                <div>
                    <TextareaAutosize
                        minRows={4}
                        placeholder={translate("session_notes")}
                        value={notes}
                        style={{
                            width: "100%",
                            backgroundColor: "#202020",
                            color: "white",
                            resize: "none",
                        }}
                        onChange={(e) => {
                            updateNote(e.target.value);
                        }}
                    />
                </div>
                <div className="session-sets-actions">
                    <Button
                        id="import-button"
                        variant="contained"
                        disabled={!changed}
                        className="full-width-button"
                        onClick={saveChanges}
                    >
                        {translate("update_session")}
                    </Button>
                </div>
            </DialogContent>
        </Dialog>
    );
}
