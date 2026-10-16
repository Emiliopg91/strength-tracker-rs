import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { BackendClient } from "@/utils/backend/client";
import { BodyMetricListItem } from "@/utils/backend/models";
import { useContext, useState } from "react";
import {
    Box,
    Button,
    Dialog,
    DialogContent,
    DialogTitle,
    IconButton,
    TextField,
} from "@mui/material";
import CloseIcon from "@mui/icons-material/Close";
import { DatePicker } from "@mui/x-date-pickers/DatePicker";

type Props = {
    latest: BodyMetricListItem | undefined;
    onClose: () => void;
};

type BodyMetricsListItemForm = Omit<
    BodyMetricListItem,
    "date" | "weight" | "fat_ratio" | "lean_mass" | "water_ratio"
> & {
    date: Date;
    weight: string;
    fat_ratio: string;
    lean_mass: string;
    water_ratio: string;
};

type NumericField = Exclude<keyof BodyMetricsListItemForm, "date">;

export function BodyMetricsAddModal({ latest, onClose }: Props) {
    const { translate, toKg, fromKg, getWeightUnit } =
        useContext(I18nSettingsContext);
    const [data, setData] = useState<BodyMetricsListItemForm>(
        latest
            ? {
                  date: new Date(),
                  weight: String(Math.round(fromKg(latest.weight) * 10) / 10),
                  fat_ratio: String(latest.fat_ratio),
                  lean_mass: String(
                      Math.round(fromKg(latest.lean_mass) * 10) / 10,
                  ),
                  water_ratio: String(latest.water_ratio),
              }
            : {
                  date: new Date(),
                  fat_ratio: "0",
                  lean_mass: "0",
                  water_ratio: "0",
                  weight: "0",
              },
    );

    const isInvalid = (prop: NumericField) => data[prop] == "";
    const hasErrors = (
        ["weight", "fat_ratio", "lean_mass", "water_ratio"] as NumericField[]
    ).some(isInvalid);

    // Invalid input is reverted on the spot. The DOM value is reset by hand because a
    // number input reports "" for things like "-", which React would not overwrite
    const onNumberChange = (
        e: React.ChangeEvent<HTMLInputElement>,
        prop: NumericField,
        max: number,
    ) => {
        const value = parseFloat(e.target.value);
        if (e.target.validity.badInput || value < 0 || value > max) {
            e.target.value = data[prop];
            return;
        }
        setData((prev) => ({ ...prev, [prop]: e.target.value }));
    };

    const onSave = () => {
        BackendClient.addBodyMeasures({
            date: Math.floor(data.date.getTime() / 1000),
            weight: toKg(parseFloat(data.weight)),
            fat_ratio: parseFloat(data.fat_ratio),
            lean_mass: toKg(parseFloat(data.lean_mass)),
            water_ratio: parseFloat(data.water_ratio),
        }).then(() => {
            onClose();
        });
    };

    const numberField = (prop: NumericField, label: string, max: number) => (
        <TextField
            label={label}
            error={isInvalid(prop)}
            type="number"
            size="small"
            fullWidth
            value={data[prop]}
            slotProps={{
                htmlInput: {
                    className: "no-spinner",
                    inputMode: "decimal",
                    min: 0,
                    max,
                    step: 0.1,
                },
            }}
            onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                onNumberChange(e, prop, max)
            }
        />
    );

    return (
        <Dialog open={true} onClose={onClose}>
            <DialogTitle>
                {translate("add_entry")}
                <IconButton onClick={onClose} className="modal-close-button">
                    <CloseIcon />
                </IconButton>
            </DialogTitle>

            <DialogContent dividers>
                <Box
                    sx={{
                        display: "flex",
                        flexDirection: "column",
                        gap: 2,
                        pt: 1,
                    }}
                >
                    <DatePicker
                        label={translate("date")}
                        value={data.date}
                        onChange={(value) => {
                            if (value != null) {
                                setData((prev) => ({ ...prev, date: value }));
                            }
                        }}
                        format="dd/MM/yyyy"
                        slotProps={{
                            textField: { size: "small", fullWidth: true },
                        }}
                    />
                    {numberField(
                        "weight",
                        translate("weight") + " (" + getWeightUnit() + ")",
                        999,
                    )}
                    {numberField("fat_ratio", translate("fat_ratio"), 100)}
                    {numberField(
                        "lean_mass",
                        translate("lean_mass") + " (" + getWeightUnit() + ")",
                        999,
                    )}
                    {numberField("water_ratio", translate("water_ratio"), 100)}
                </Box>
                <hr />
                <div>
                    <Button
                        id="save-measure-button"
                        variant="contained"
                        className="full-width-button"
                        disabled={hasErrors}
                        onClick={onSave}
                    >
                        {translate("save")}
                    </Button>
                </div>
            </DialogContent>
        </Dialog>
    );
}
