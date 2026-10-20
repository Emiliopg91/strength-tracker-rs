import { I18nSettingsContext } from "@/context/I18nSettingsContext";
import { useContext, useState } from "react";
import {
    Box,
    Button,
    Dialog,
    DialogContent,
    DialogTitle,
    IconButton,
} from "@mui/material";
import CloseIcon from "@mui/icons-material/Close";
import { DatePicker } from "@mui/x-date-pickers";
import { BackendClient } from "@/utils/backend/client";
import { useLoadingTask } from "@/hooks/useLoadingTask";

type Props = {
    onClose: () => void;
};

export function ReportModal({ onClose }: Props) {
    const { translate } = useContext(I18nSettingsContext);
    const loadingTask = useLoadingTask();

    const [from, setFrom] = useState(() => {
        const date = new Date();
        date.setHours(0, 0, 0, 0);
        date.setDate(date.getDate() - 14);
        return date;
    });
    const [to, setTo] = useState(() => {
        const date = new Date();
        date.setHours(0, 0, 0, 0);
        return date;
    });
    const [today] = useState(() => {
        const date = new Date();
        date.setHours(0, 0, 0, 0);
        return date;
    });

    const generateReport = () => {
        loadingTask(
            BackendClient.loadReport(
                from.getTime() / 1000,
                to.getTime() / 1000 + 24 * 60 * 60 - 1,
            ).finally(onClose),
        );
    };

    return (
        <Dialog open={true} onClose={onClose} fullWidth maxWidth="sm">
            <DialogTitle>
                {translate("generate_report")}
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
                    <Box
                        sx={{
                            display: "flex",
                            flexDirection: "row",
                            gap: 2,
                            pt: 1,
                        }}
                    >
                        <DatePicker
                            label={translate("begin_date")}
                            value={from}
                            maxDate={
                                today.getTime() > to.getTime() ? to : today
                            }
                            onChange={(value) => {
                                if (value != null) {
                                    setFrom(value);
                                }
                            }}
                            format="dd/MM/yyyy"
                            slotProps={{
                                textField: { size: "small", fullWidth: true },
                            }}
                        />
                        <DatePicker
                            label={translate("end_date")}
                            value={to}
                            minDate={from}
                            maxDate={today}
                            onChange={(value) => {
                                if (value != null) {
                                    setTo(value);
                                }
                            }}
                            format="dd/MM/yyyy"
                            slotProps={{
                                textField: { size: "small", fullWidth: true },
                            }}
                        />
                    </Box>
                    <Button variant="contained" onClick={generateReport}>
                        {translate("generate")}
                    </Button>
                </Box>
            </DialogContent>
        </Dialog>
    );
}
