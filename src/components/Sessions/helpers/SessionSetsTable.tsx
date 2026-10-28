import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { SessionSet } from "@/utils/backend/models";
import { Fragment, useContext, useState } from "react";
import { FormControlLabel, Switch } from "@mui/material";
import EmojiEventsIcon from "@mui/icons-material/EmojiEvents";
import { AppContext, ExerciseOption } from "@/context/AppContext";
import { ExerciseField } from "@/components/Fields/ExerciseField";
import { RepetitionsField } from "@/components/Fields/RepetitionsField";
import { WeightField } from "@/components/Fields/WeightField";
import { SessionUtils } from "@/utils/SessionUtils";

type Props = {
    series: SessionSet[];
    onUpdateSerieRepsWeight: (
        idx: number,
        field: "reps" | "weight",
        value: number,
    ) => void;
    onUpdateSerieExercise: (idx: number, category: number, id: number) => void;
};

export function SessionSetsTable({
    series,
    onUpdateSerieRepsWeight,
    onUpdateSerieExercise,
}: Props) {
    const { translate, formatDuration } = useContext(I18nSettingsContext);
    // Without catalog the exercise name is rendered as plain text instead of a selector
    const { exercisesCatalog } = useContext(AppContext);

    const [hasRests] = useState(series.find((s) => s.rest) !== undefined);

    const [groupedSets, setGroupedSets] = useState(false);

    const updateSerieReps = (idx: number, reps: number | null) => {
        onUpdateSerieRepsWeight(idx, "reps", reps ?? 0);
    };

    const updateSerieRepsOld = (idx: number, reps: number | null) => {
        onUpdateSerieRepsWeight(idx, "reps", reps ?? 0);
    };

    const updateSerieWeight = (idx: number, weight: number | null) => {
        onUpdateSerieRepsWeight(idx, "weight", weight ?? 0);
    };

    const updateSerieWeightOld = (idx: number, weight: number | null) => {
        onUpdateSerieRepsWeight(idx, "weight", weight ?? 0);
    };

    const onExerciseChange = (idx: number, exerciseOption: ExerciseOption) => {
        onUpdateSerieExercise(idx, exerciseOption.ex_cat, exerciseOption.ex_id);
    };

    const onExerciseChangeOld = (
        idx: number,
        exerciseOption: ExerciseOption,
    ) => {
        onUpdateSerieExercise(idx, exerciseOption.ex_cat, exerciseOption.ex_id);
    };

    const groupedBySerie = () => {
        let serieIdx = 1;
        let firstOfSerie = true;
        return (
            <>
                {[...series]
                    .sort((a, b) => a.idx - b.idx)
                    .map((serie, idx) => {
                        const selected = exercisesCatalog.find(
                            (e) =>
                                e.ex_cat == serie.ex_cat &&
                                e.ex_id == serie.ex_id,
                        );
                        const isFirst = firstOfSerie;
                        const thisSerie = serieIdx;
                        firstOfSerie = serie.rest;
                        if (serie.rest) {
                            serieIdx += 1;
                        }

                        return (
                            <Fragment key={serie.idx}>
                                {isFirst && (
                                    <thead>
                                        <tr className="divider-bottom">
                                            <th colSpan={2}>
                                                {translate("set_x", [
                                                    thisSerie.toString(),
                                                ])}
                                            </th>
                                        </tr>
                                    </thead>
                                )}

                                <tbody>
                                    <tr>
                                        {serie.rest && (
                                            <td colSpan={2}>
                                                <div
                                                    style={{
                                                        textAlign: "center",
                                                        paddingTop: "10px",
                                                    }}
                                                >
                                                    <span>
                                                        <small>
                                                            {"" +
                                                                translate(
                                                                    "step_type_rest",
                                                                ) +
                                                                ": "}
                                                            {formatDuration(
                                                                serie.duration,
                                                            )}
                                                        </small>
                                                    </span>
                                                </div>
                                                <div className="divider-bottom session-sets-rest-divider" />
                                            </td>
                                        )}

                                        {!serie.rest && (
                                            <>
                                                <td>
                                                    {!selected && (
                                                        <>
                                                            {translate(
                                                                "exercise_" +
                                                                    serie.ex_cat +
                                                                    "_" +
                                                                    serie.ex_id,
                                                            )}
                                                        </>
                                                    )}
                                                    {selected && (
                                                        <ExerciseField
                                                            selected={selected}
                                                            onChange={(opt) =>
                                                                onExerciseChange(
                                                                    idx,
                                                                    opt,
                                                                )
                                                            }
                                                            sx={{
                                                                width: 300,
                                                                mt: 1,
                                                            }}
                                                        />
                                                    )}
                                                </td>
                                                <td>
                                                    <RepetitionsField
                                                        label="repetitions"
                                                        allowEmpty
                                                        value={serie.reps}
                                                        sx={{
                                                            width: 100,
                                                            mt: 1,
                                                        }}
                                                        style={{
                                                            marginTop: "5px",
                                                        }}
                                                        onChange={(e) => {
                                                            updateSerieReps(
                                                                idx,
                                                                e,
                                                            );
                                                        }}
                                                    />
                                                    <WeightField
                                                        allowEmpty
                                                        sx={{
                                                            width: 100,
                                                            px: 1,
                                                            mt: 1,
                                                        }}
                                                        style={{
                                                            marginTop: "5px",
                                                        }}
                                                        value={serie.weight}
                                                        onChange={(e) => {
                                                            updateSerieWeight(
                                                                idx,
                                                                e,
                                                            );
                                                        }}
                                                    />
                                                    {serie.pr && (
                                                        <EmojiEventsIcon
                                                            fontSize="small"
                                                            className="trophy-icon"
                                                            style={{
                                                                marginTop:
                                                                    "15px",
                                                            }}
                                                        />
                                                    )}
                                                </td>
                                            </>
                                        )}
                                    </tr>
                                </tbody>
                            </Fragment>
                        );
                    })}
            </>
        );
    };

    const groupedByExercises = () => {
        const { exercises, groupedSeries } =
            SessionUtils.groupByExercises(series);
        return (
            <>
                <thead>
                    <tr className="divider-bottom">
                        <th>{translate("exercise")}</th>
                        <th>{translate("series")}</th>
                    </tr>
                </thead>
                <tbody>
                    {exercises.map((exercise) =>
                        groupedSeries[exercise].map(([idxS, serie], idx) => {
                            const selected =
                                idx === 0
                                    ? exercisesCatalog.find(
                                          (e) =>
                                              e.ex_cat == serie.ex_cat &&
                                              e.ex_id == serie.ex_id,
                                      )
                                    : undefined;
                            return (
                                <tr key={`${exercise}-${idx}`}>
                                    {idx === 0 && (
                                        <td
                                            className="divider-bottom"
                                            rowSpan={
                                                groupedSeries[exercise].length
                                            }
                                        >
                                            {!selected && (
                                                <>
                                                    {translate(
                                                        "exercise_" +
                                                            serie.ex_cat +
                                                            "_" +
                                                            serie.ex_id,
                                                    )}
                                                </>
                                            )}
                                            {selected && (
                                                <ExerciseField
                                                    selected={selected}
                                                    onChange={(opt) =>
                                                        onExerciseChangeOld(
                                                            idxS,
                                                            opt,
                                                        )
                                                    }
                                                    sx={{
                                                        width: 300,
                                                    }}
                                                />
                                            )}
                                        </td>
                                    )}

                                    <td
                                        className={[
                                            idx ===
                                            groupedSeries[exercise].length - 1
                                                ? "divider-bottom group-cell-last"
                                                : undefined,
                                            idx === 0
                                                ? "group-cell-first"
                                                : undefined,
                                        ]
                                            .filter(Boolean)
                                            .join(" ")}
                                    >
                                        <RepetitionsField
                                            label="repetitions"
                                            allowEmpty
                                            value={serie.reps}
                                            sx={{ width: 100 }}
                                            style={{ marginTop: "5px" }}
                                            onChange={(e) => {
                                                updateSerieRepsOld(idxS, e);
                                            }}
                                        />
                                        <WeightField
                                            allowEmpty
                                            sx={{ width: 100, px: 1 }}
                                            style={{ marginTop: "5px" }}
                                            value={serie.weight}
                                            onChange={(e) => {
                                                updateSerieWeightOld(idxS, e);
                                            }}
                                        />
                                        {serie.pr && (
                                            <EmojiEventsIcon
                                                className="trophy-icon"
                                                style={{ marginTop: "15px" }}
                                            />
                                        )}
                                    </td>
                                </tr>
                            );
                        }),
                    )}
                </tbody>
            </>
        );
    };

    return (
        <div className="session-sets-container">
            {hasRests && (
                <div
                    style={{
                        display: "flex",
                        justifyContent: "flex-end",
                        width: "100%",
                    }}
                >
                    <FormControlLabel
                        control={
                            <Switch
                                size="small"
                                checked={!groupedSets}
                                onChange={(e) =>
                                    setGroupedSets(!e.target.checked)
                                }
                            />
                        }
                        label={translate("group_by_exercises")}
                    />
                </div>
            )}
            <table>
                <colgroup>
                    <col className="col-320" />
                    <col className="col-230" />
                </colgroup>
                {groupedSets && groupedBySerie()}
                {!groupedSets && groupedByExercises()}
            </table>
        </div>
    );
}
