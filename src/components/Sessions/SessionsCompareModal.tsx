import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { Fragment, useContext, useMemo } from "react";
import { Dialog, DialogContent, DialogTitle, IconButton } from "@mui/material";
import CloseIcon from "@mui/icons-material/Close";
import { SessionFrontDetails } from "@/utils/SessionUtils";
import { SessionSet } from "@/utils/backend/models";

type Props = {
    sessions: SessionFrontDetails[];
    onClose: () => void;
};

export function SessionsCompareModal({ sessions, onClose }: Props) {
    const {
        translate,
        fromKg,
        getWeightUnit,
        formatDuration,
        formatTimeDate,
        formatNumber,
    } = useContext(I18nSettingsContext);
    // Non-rest sets grouped by exercise ("cat-id"); exercises follow the order
    // of their first set across sessions
    const { exercises, groupedSets } = useMemo(() => {
        const exercises: string[] = [];
        const groupedSets = sessions.map((s) => {
            const groups: Record<string, SessionSet[]> = {};
            s.sets
                .filter((set) => !set.rest)
                .sort((a, b) => a.idx - b.idx)
                .forEach((set) => {
                    const key = set.ex_cat + "-" + set.ex_id;
                    (groups[key] ??= []).push(set);
                });
            return groups;
        });

        let pos = 0;
        while (true) {
            let any = false;
            for (const groups of groupedSets) {
                const keys = Object.keys(groups);
                if (keys.length > pos) {
                    if (!exercises.includes(keys[pos])) {
                        exercises.push(keys[pos]);
                    }
                    any = true;
                }
            }
            if (!any) break;
            pos++;
        }
        return { exercises, groupedSets };
    }, [sessions]);

    return (
        <Dialog open={true} onClose={onClose}>
            <DialogTitle>
                {translate("sessions_comparaison") + ": " + sessions[0].name}
                <IconButton onClick={onClose} className="modal-close-button">
                    <CloseIcon />
                </IconButton>
            </DialogTitle>

            <DialogContent dividers>
                <table id="sessions-compare-table">
                    <colgroup>
                        <col className="col-230" />
                        <col className="col-180" />
                        <col className="col-180" />
                    </colgroup>

                    <tbody>
                        <tr>
                            <td
                                style={{ borderBottom: "1px solid white" }}
                            ></td>
                            <td style={{ borderBottom: "1px solid white" }}>
                                <b>{formatTimeDate(sessions[0].timestamp)}</b>
                            </td>
                            <td style={{ borderBottom: "1px solid white" }}>
                                <b>{formatTimeDate(sessions[1].timestamp)}</b>
                            </td>
                        </tr>
                        <tr>
                            <td>{translate("time")}</td>
                            <td>
                                {formatDuration(sessions[0].active_time) +
                                    " / " +
                                    formatDuration(
                                        sessions[0].total_elapsed_time,
                                    )}
                            </td>
                            <td>
                                {formatDuration(sessions[1].active_time) +
                                    " / " +
                                    formatDuration(
                                        sessions[1].total_elapsed_time,
                                    )}
                            </td>
                        </tr>
                        <tr>
                            <td>{translate("calories")}</td>
                            <td>
                                {formatNumber(sessions[0].active_calories, 0)}{" "}
                                Kcal
                            </td>
                            <td>
                                {formatNumber(sessions[1].active_calories, 0)}{" "}
                                Kcal
                            </td>
                        </tr>
                        <tr>
                            <td>{translate("volume")}</td>
                            <td>
                                {formatNumber(fromKg(sessions[0].volume), 1)}{" "}
                                {getWeightUnit()}
                            </td>
                            <td>
                                {formatNumber(fromKg(sessions[1].volume), 1)}{" "}
                                {getWeightUnit()}
                            </td>
                        </tr>
                        <tr>
                            <td>{translate("workout_load")}</td>
                            <td>
                                {formatNumber(sessions[0].training_load, 0)}
                            </td>
                            <td>
                                {formatNumber(sessions[1].training_load, 0)}
                            </td>
                        </tr>
                        <tr
                            style={{
                                textAlign: "center",
                                paddingTop: "20px",
                                fontWeight: "bolder",
                            }}
                        >
                            <td style={{ borderBottom: "1px solid white" }}>
                                <br />
                                {translate("exercise")}
                            </td>
                            <td
                                style={{ borderBottom: "1px solid white" }}
                                colSpan={2}
                            >
                                <br />
                                {translate("series")}
                            </td>
                        </tr>
                        {exercises.map((exercise) => (
                            <tr
                                key={`exercise-${exercise}`}
                                style={{ textAlign: "center" }}
                            >
                                <td className="divider-bottom-white">
                                    {translate(
                                        "exercise_" +
                                            exercise.replace("-", "_"),
                                    )}
                                </td>
                                {groupedSets.map((groups, idx) => (
                                    <td
                                        key={`session-${idx}`}
                                        className="divider-bottom-white"
                                    >
                                        {groups[exercise]?.map((set) => (
                                            <Fragment
                                                key={`session-${idx}-set-${set.idx}`}
                                            >
                                                {set.reps +
                                                    " x " +
                                                    formatNumber(
                                                        set.weight,
                                                        1,
                                                    ) +
                                                    " " +
                                                    getWeightUnit()}
                                                <br />
                                            </Fragment>
                                        ))}
                                    </td>
                                ))}
                            </tr>
                        ))}
                    </tbody>
                </table>
            </DialogContent>
        </Dialog>
    );
}
