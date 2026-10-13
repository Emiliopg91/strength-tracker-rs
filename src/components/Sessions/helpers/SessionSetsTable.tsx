import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { SessionSet } from "@/utils/backend/models";
import { Fragment, useContext, useMemo, useState } from "react";
import {
  Autocomplete,
  FormControlLabel,
  Switch,
  TextField,
} from "@mui/material";
import EmojiEventsIcon from "@mui/icons-material/EmojiEvents";
import { ExerciseOption, useExerciseCatalog } from "@/hooks/useExerciseCatalog";

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
  const { translate, getWeightUnit, formatDuration } =
    useContext(I18nSettingsContext);
  // On failure the exercise name is rendered as plain text instead of a selector
  const exercisesCatalog = useExerciseCatalog(() => {
    console.warn("Could not load exercises catalog");
  });

  const [hasRests] = useState(series.find((s) => s.rest) !== undefined);

  const [groupedSets, setGroupedSets] = useState(
    series.find((s) => s.rest) !== undefined,
  );

  const updateSerieReps = (idx: number, newVal: string) => {
    let reps = parseInt(newVal);
    if (isNaN(reps)) {
      reps = 0;
    }
    onUpdateSerieRepsWeight(idx, "reps", reps);
  };

  const updateSerieRepsOld = (idx: number, newVal: string) => {
    let reps = parseInt(newVal);
    if (isNaN(reps)) {
      reps = 0;
    }
    onUpdateSerieRepsWeight(idx, "reps", reps);
  };

  const updateSerieWeight = (idx: number, newVal: string) => {
    let weight = parseFloat(newVal);
    if (isNaN(weight)) {
      weight = 0;
    }
    onUpdateSerieRepsWeight(idx, "weight", weight);
  };

  const updateSerieWeightOld = (idx: number, newVal: string) => {
    let weight = parseFloat(newVal);
    if (isNaN(weight)) {
      weight = 0;
    }
    onUpdateSerieRepsWeight(idx, "weight", weight);
  };

  const onExerciseChange = (idx: number, exerciseOption: ExerciseOption) => {
    onUpdateSerieExercise(idx, exerciseOption.ex_cat, exerciseOption.ex_id);
  };

  const onExerciseChangeOld = (idx: number, exerciseOption: ExerciseOption) => {
    onUpdateSerieExercise(idx, exerciseOption.ex_cat, exerciseOption.ex_id);
  };

  const [exercises, groupedSeries] = useMemo(() => {
    const exercises: string[] = [];
    const groupedSeries: Record<string, [number, SessionSet][]> = {};

    let idx = 0;
    for (let i = 0; i < series.length; i++) {
      if (!series[i].rest) {
        const ex_key = series[i].ex_cat + "-" + series[i].ex_id;
        if (!exercises.includes(ex_key)) {
          exercises.push(ex_key);
        }

        if (groupedSeries[ex_key] === undefined) {
          groupedSeries[ex_key] = [];
        }

        groupedSeries[ex_key].push([idx, series[i]]);
      }
      idx++;
    }

    return [exercises, groupedSeries];
  }, [series]);

  const groupedBySerie = () => {
    let serieIdx = 1;
    let firstOfSerie = true;
    return (
      <>
        {[...series]
          .sort((a, b) => a.idx - b.idx)
          .map((serie, idx) => {
            const selected = exercisesCatalog.find(
              (e) => e.ex_cat == serie.ex_cat && e.ex_id == serie.ex_id,
            );
            const isFirst = firstOfSerie;
            const thisSerie = serieIdx;
            firstOfSerie = serie.rest;
            if (serie.rest) {
              serieIdx++;
            }

            return (
              <Fragment key={serie.idx}>
                {isFirst && (
                  <thead>
                    <tr className="divider-bottom">
                      <th colSpan={2}>
                        {translate("set_x", [thisSerie.toString()])}
                      </th>
                    </tr>
                  </thead>
                )}

                <tbody>
                  <tr>
                    {serie.rest && (
                      <td colSpan={2}>
                        <div
                          style={{ textAlign: "center", paddingTop: "10px" }}
                        >
                          <span>
                            <small>
                              {"" + translate("step_type_rest") + ": "}
                              {formatDuration(serie.duration)}
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
                                "exercise_" + serie.ex_cat + "_" + serie.ex_id,
                              )}
                            </>
                          )}
                          {selected && (
                            <Autocomplete
                              size="small"
                              sx={{ width: 300, mt: 1 }}
                              options={exercisesCatalog}
                              value={selected}
                              disableClearable
                              groupBy={(opt) =>
                                translate("exercise_" + opt.ex_cat)
                              }
                              getOptionKey={(opt) =>
                                `${opt.ex_cat}-${opt.ex_id}`
                              }
                              isOptionEqualToValue={(a, b) =>
                                a.ex_cat == b.ex_cat && a.ex_id == b.ex_id
                              }
                              onChange={(_, opt) => onExerciseChange(idx, opt)}
                              renderInput={(params) => (
                                <TextField
                                  {...params}
                                  multiline
                                  label={translate("select_exercise")}
                                />
                              )}
                            />
                          )}
                        </td>
                        <td>
                          <TextField
                            label={translate("repetitions")}
                            type="number"
                            size="small"
                            value={serie.reps}
                            sx={{ width: 100, mt: 1 }}
                            style={{ marginTop: "5px" }}
                            slotProps={{
                              htmlInput: {
                                className: "no-spinner",
                                min: 0,
                              },
                            }}
                            onChange={(e) => {
                              updateSerieReps(idx, e.target.value);
                            }}
                          />
                          <TextField
                            label={
                              translate("weight") + " (" + getWeightUnit() + ")"
                            }
                            type="number"
                            size="small"
                            sx={{ width: 100, px: 1, mt: 1 }}
                            style={{ marginTop: "5px" }}
                            value={serie.weight?.toString()}
                            slotProps={{
                              htmlInput: {
                                className: "no-spinner",
                                min: 0,
                              },
                            }}
                            onChange={(e) => {
                              updateSerieWeight(idx, e.target.value);
                            }}
                          />
                          {serie.pr && (
                            <EmojiEventsIcon
                              fontSize="small"
                              className="trophy-icon"
                              style={{ marginTop: "15px" }}
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
                      (e) => e.ex_cat == serie.ex_cat && e.ex_id == serie.ex_id,
                    )
                  : undefined;
              return (
                <tr key={`${exercise}-${idx}`}>
                  {idx === 0 && (
                    <td
                      className="divider-bottom"
                      rowSpan={groupedSeries[exercise].length}
                    >
                      {!selected && (
                        <>
                          {translate(
                            "exercise_" + serie.ex_cat + "_" + serie.ex_id,
                          )}
                        </>
                      )}
                      {selected && (
                        <Autocomplete
                          size="small"
                          sx={{ width: 300 }}
                          options={exercisesCatalog}
                          value={selected}
                          disableClearable
                          groupBy={(opt) => translate("exercise_" + opt.ex_cat)}
                          getOptionKey={(opt) => `${opt.ex_cat}-${opt.ex_id}`}
                          isOptionEqualToValue={(a, b) =>
                            a.ex_cat == b.ex_cat && a.ex_id == b.ex_id
                          }
                          onChange={(_, opt) => onExerciseChangeOld(idxS, opt)}
                          renderInput={(params) => (
                            <TextField
                              {...params}
                              multiline
                              label={translate("select_exercise")}
                            />
                          )}
                        />
                      )}
                    </td>
                  )}

                  <td
                    className={[
                      idx === groupedSeries[exercise].length - 1
                        ? "divider-bottom group-cell-last"
                        : undefined,
                      idx === 0 ? "group-cell-first" : undefined,
                    ]
                      .filter(Boolean)
                      .join(" ")}
                  >
                    <TextField
                      label={translate("repetitions")}
                      type="number"
                      size="small"
                      value={serie.reps}
                      sx={{ width: 100 }}
                      style={{ marginTop: "5px" }}
                      slotProps={{
                        htmlInput: {
                          className: "no-spinner",
                          min: 0,
                        },
                      }}
                      onChange={(e) => {
                        updateSerieRepsOld(idxS, e.target.value);
                      }}
                    />
                    <TextField
                      label={translate("weight") + " (" + getWeightUnit() + ")"}
                      type="number"
                      size="small"
                      sx={{ width: 100, px: 1 }}
                      style={{ marginTop: "5px" }}
                      value={serie.weight?.toString()}
                      slotProps={{
                        htmlInput: {
                          className: "no-spinner",
                          min: 0,
                        },
                      }}
                      onChange={(e) => {
                        updateSerieWeightOld(idxS, e.target.value);
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
          style={{ display: "flex", justifyContent: "flex-end", width: "100%" }}
        >
          <FormControlLabel
            control={
              <Switch
                size="small"
                checked={!groupedSets}
                onChange={(e) => setGroupedSets(!e.target.checked)}
              />
            }
            label={translate("group_by_exercises")}
          />
        </div>
      )}
      <table hidden={!groupedSets}>
        <colgroup>
          <col className="col-320" />
          <col className="col-230" />
        </colgroup>
        {groupedBySerie()}
      </table>
      <table hidden={groupedSets}>
        <colgroup>
          <col className="col-320" />
          <col className="col-230" />
        </colgroup>
        {groupedByExercises()}
      </table>
    </div>
  );
}
